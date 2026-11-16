//! HTTP access to Wikidata and Overpass: shared client, timeouts, politeness.

use std::time::{Duration, Instant};

use serde_json::Value;
use tokio::sync::Mutex;
use url::Url;

use super::overpass::{self, StoreLocation};
use super::wikidata::{self, BrandCandidate};
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
/// Labels and search results in the users' language.
pub const LANGUAGE: &str = "it";

#[derive(Debug, thiserror::Error)]
pub enum UpstreamError {
    #[error("request failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("unexpected response: {0}")]
    Response(String),
}

pub struct Upstream {
    http: reqwest::Client,
    overpass_url: Url,
    wikidata_url: Url,
    region: String,
    /// When the last Overpass query finished; also serialises queries.
    overpass_gate: Mutex<Option<Instant>>,
}

impl Upstream {
    pub fn new(config: &Config) -> Result<Self, reqwest::Error> {
        // reqwest is built without a default crypto provider (no C toolchain for aws-lc).
        let _ = rustls::crypto::ring::default_provider().install_default();
        let http = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .connect_timeout(Duration::from_secs(10))
            .build()?;
        Ok(Self {
            http,
            overpass_url: config.overpass_url.clone(),
            wikidata_url: config.wikidata_url.clone(),
            region: config.region_iso3166.clone(),
            overpass_gate: Mutex::new(None),
        })
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

    async fn wikidata_get(&self, params: &[(&str, String)]) -> Result<Value, UpstreamError> {
        let mut url = self.wikidata_url.clone();
        url.query_pairs_mut()
            .extend_pairs(params.iter().map(|(key, value)| (*key, value.as_str())));
        let response = self
            .http
            .get(url)
            .timeout(WIKIDATA_TIMEOUT)
            .send()
            .await?
            .error_for_status()?;
        Ok(response.json().await?)
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
        let body: Value = response.json().await?;
        if body.get("elements").is_none() {
            let remark = body["remark"].as_str().unwrap_or("no elements in response");
            return Err(UpstreamError::Response(remark.to_owned()));
        }
        Ok(overpass::parse(&body))
    }
}
