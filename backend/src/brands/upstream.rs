//! HTTP access to Wikidata, Commons and Overpass: shared client, timeouts, politeness.

use std::collections::HashSet;
use std::time::{Duration, Instant};

use serde_json::Value;
use tokio::sync::Mutex;
use url::Url;
use uuid::Uuid;

use super::logo;
use super::overpass::{self, StoreLocation};
use super::wikidata::{self, BrandCandidate};
use crate::cards::images::ImageMime;
use crate::config::Config;

/// Generic on purpose: identifies the software, not the person running it.
const USER_AGENT: &str = concat!(
    "tessera/",
    env!("CARGO_PKG_VERSION"),
    " (self-hosted loyalty card app)"
);
const WIKIDATA_TIMEOUT: Duration = Duration::from_secs(10);
const OVERPASS_TIMEOUT: Duration = Duration::from_secs(overpass::QUERY_TIMEOUT_SECS as u64 + 30);
/// Overpass asks clients not to hammer it; one query at a time, spaced out.
const OVERPASS_MIN_GAP: Duration = Duration::from_secs(5);
/// Wikidata answers are a few KB; anything bigger is not what we asked for.
const MAX_WIKIDATA_BYTES: usize = 1024 * 1024;
/// A country-wide brand is a few MB of JSON at most.
const MAX_OVERPASS_BYTES: usize = 32 * 1024 * 1024;
/// Labels and search results in the users' language.
pub const LANGUAGE: &str = "it";

#[derive(Debug, thiserror::Error)]
pub enum UpstreamError {
    #[error("request failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("unexpected response: {0}")]
    Response(String),
}

/// A logo downloaded from Commons, checked to be a raster image.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FetchedLogo {
    pub mime: ImageMime,
    pub data: Vec<u8>,
}

pub struct Upstream {
    http: reqwest::Client,
    overpass_url: Url,
    wikidata_url: Url,
    commons_url: Url,
    region: String,
    /// When the last Overpass query finished; also serialises queries.
    overpass_gate: Mutex<Option<Instant>>,
    /// Brands with a first fetch queued or running, so it is not queued twice.
    pending_first_fetch: std::sync::Mutex<HashSet<Uuid>>,
}

impl Upstream {
    pub fn new(config: &Config) -> Result<Self, reqwest::Error> {
        // reqwest is built without a default crypto provider (no C toolchain for aws-lc).
        let _ = rustls::crypto::ring::default_provider().install_default();
        let http = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            // The endpoints are fixed in config; never let a response send us elsewhere.
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(10))
            .build()?;
        Ok(Self {
            http,
            overpass_url: config.overpass_url.clone(),
            wikidata_url: config.wikidata_url.clone(),
            commons_url: config.commons_url.clone(),
            region: config.region_iso3166.clone(),
            overpass_gate: Mutex::new(None),
            pending_first_fetch: std::sync::Mutex::new(HashSet::new()),
        })
    }

    /// Claims the first fetch of `brand_id`; `false` if one is already queued.
    pub fn claim_first_fetch(&self, brand_id: Uuid) -> bool {
        self.pending_first_fetch
            .lock()
            .map(|mut pending| pending.insert(brand_id))
            .unwrap_or(false)
    }

    pub fn release_first_fetch(&self, brand_id: Uuid) {
        if let Ok(mut pending) = self.pending_first_fetch.lock() {
            pending.remove(&brand_id);
        }
    }

    pub async fn search_brands(&self, query: &str) -> Result<Vec<BrandCandidate>, UpstreamError> {
        let body = self
            .wikidata_get(&wikidata::search_params(query, LANGUAGE))
            .await?;
        Ok(wikidata::parse_search(&body))
    }

    /// The brand's name, or `None` when Wikidata has no such item.
    pub async fn brand_label(&self, qid: &str) -> Result<Option<String>, UpstreamError> {
        let body = self
            .wikidata_get(&wikidata::label_params(qid, LANGUAGE))
            .await?;
        Ok(wikidata::parse_label(&body, qid, LANGUAGE))
    }

    /// The brand's current logo, or `None` when Wikidata lists none or
    /// Commons cannot render it as a raster thumbnail.
    pub async fn brand_logo(&self, qid: &str) -> Result<Option<FetchedLogo>, UpstreamError> {
        let claims = self.wikidata_get(&logo::claim_params(qid)).await?;
        let Some(file) = logo::parse_logo_file(&claims) else {
            return Ok(None);
        };
        let info = self
            .api_get(&self.commons_url, &logo::thumb_params(&file))
            .await?;
        let Some(thumb_url) = logo::parse_thumb_url(&info, &self.commons_url) else {
            return Ok(None);
        };
        let response = self
            .http
            .get(thumb_url)
            .timeout(WIKIDATA_TIMEOUT)
            .send()
            .await?
            .error_for_status()?;
        let data = read_limited(response, logo::MAX_LOGO_BYTES).await?;
        let mime = ImageMime::sniff(&data)
            .ok_or_else(|| UpstreamError::Response("logo is not a JPEG, PNG or WebP".into()))?;
        Ok(Some(FetchedLogo { mime, data }))
    }

    async fn wikidata_get(&self, params: &[(&str, String)]) -> Result<Value, UpstreamError> {
        self.api_get(&self.wikidata_url, params).await
    }

    /// A GET to a MediaWiki `api.php` (Wikidata or Commons).
    async fn api_get(&self, base: &Url, params: &[(&str, String)]) -> Result<Value, UpstreamError> {
        let mut url = base.clone();
        url.query_pairs_mut()
            .extend_pairs(params.iter().map(|(key, value)| (*key, value.as_str())));
        let response = self
            .http
            .get(url)
            .timeout(WIKIDATA_TIMEOUT)
            .send()
            .await?
            .error_for_status()?;
        read_json(response, MAX_WIKIDATA_BYTES).await
    }

    /// Every location of the brand in the configured country.
    pub async fn store_locations(&self, qid: &str) -> Result<Vec<StoreLocation>, UpstreamError> {
        let mut last = self.overpass_gate.lock().await;
        if let Some(wait) = last.and_then(|at| OVERPASS_MIN_GAP.checked_sub(at.elapsed())) {
            tokio::time::sleep(wait).await;
        }
        let result = self
            .overpass_query(&overpass::query(qid, &self.region))
            .await;
        *last = Some(Instant::now());
        result
    }

    async fn overpass_query(&self, query: &str) -> Result<Vec<StoreLocation>, UpstreamError> {
        let form: String = url::form_urlencoded::Serializer::new(String::new())
            .append_pair("data", query)
            .finish();
        let response = self
            .http
            .post(self.overpass_url.clone())
            .header(
                reqwest::header::CONTENT_TYPE,
                "application/x-www-form-urlencoded",
            )
            .body(form)
            .timeout(OVERPASS_TIMEOUT)
            .send()
            .await?
            .error_for_status()?;
        let body = read_json(response, MAX_OVERPASS_BYTES).await?;
        // A `remark` means the query was cut short (timeout, memory): the list is partial.
        if let Some(remark) = body["remark"].as_str() {
            return Err(UpstreamError::Response(remark.to_owned()));
        }
        if body.get("elements").is_none() {
            return Err(UpstreamError::Response("no elements in response".into()));
        }
        Ok(overpass::parse(&body))
    }
}

/// Reads at most `limit` bytes of JSON, failing instead of buffering more.
async fn read_json(response: reqwest::Response, limit: usize) -> Result<Value, UpstreamError> {
    let body = read_limited(response, limit).await?;
    serde_json::from_slice(&body).map_err(|e| UpstreamError::Response(e.to_string()))
}

/// Reads at most `limit` bytes of body, failing instead of buffering more.
async fn read_limited(
    mut response: reqwest::Response,
    limit: usize,
) -> Result<Vec<u8>, UpstreamError> {
    let too_large = || UpstreamError::Response(format!("response larger than {limit} bytes"));
    if response
        .content_length()
        .is_some_and(|length| length > limit as u64)
    {
        return Err(too_large());
    }
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        if body.len() + chunk.len() > limit {
            return Err(too_large());
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}
