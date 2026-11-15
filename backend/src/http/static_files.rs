//! Serves the Vite build embedded in the binary, with an SPA fallback.

use std::borrow::Cow;

use axum::body::Body;
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};
use rust_embed::Embed;

use crate::error::ApiError;

/// `allow_missing` lets the backend compile before the frontend is built;
/// requests then get a 503 explaining what is missing.
#[derive(Embed)]
#[folder = "../frontend/dist"]
#[allow_missing = true]
struct EmbeddedAssets;

const INDEX: &str = "index.html";
/// Vite emits content-hashed files here, so they can be cached forever.
const HASHED_ASSET_PREFIX: &str = "assets/";
const IMMUTABLE: &str = "public, max-age=31536000, immutable";
/// index.html, sw.js and the manifest must be revalidated so updates reach the phone.
const REVALIDATE: &str = "no-cache";

pub struct Asset {
    pub data: Cow<'static, [u8]>,
    pub sha256: [u8; 32],
}

/// Where static files come from: the embedded build in production, a map in tests.
pub trait AssetSource {
    fn get(&self, path: &str) -> Option<Asset>;
}

struct Embedded;

impl AssetSource for Embedded {
    fn get(&self, path: &str) -> Option<Asset> {
        EmbeddedAssets::get(path).map(|file| Asset {
            sha256: file.metadata.sha256_hash(),
            data: file.data,
        })
    }
}

pub async fn serve(method: Method, uri: Uri, headers: HeaderMap) -> Response {
    respond(&Embedded, &method, uri.path(), &headers)
}

fn respond(
    source: &impl AssetSource,
    method: &Method,
    path: &str,
    headers: &HeaderMap,
) -> Response {
    let path = path.trim_start_matches('/');
    // `/api/` and other unmatched API paths must never receive index.html.
    if path == "api" || path.starts_with("api/") {
        return ApiError::NotFound.into_response();
    }
    if method != Method::GET && method != Method::HEAD {
        return StatusCode::METHOD_NOT_ALLOWED.into_response();
    }

    let (target, asset) = match source.get(path).filter(|_| !path.is_empty()) {
        Some(asset) => (path, Some(asset)),
        None => match fallback_for(path) {
            Fallback::Index => (INDEX, source.get(INDEX)),
            Fallback::NotFound => return StatusCode::NOT_FOUND.into_response(),
        },
    };
    let Some(asset) = asset else {
        return (StatusCode::SERVICE_UNAVAILABLE, "frontend not built").into_response();
    };

    let etag = format!("\"{}\"", hex(&asset.sha256));
    let status = if etag_matches(headers, &etag) {
        StatusCode::NOT_MODIFIED
    } else {
        StatusCode::OK
    };
    let body = match (status, asset.data) {
        (StatusCode::NOT_MODIFIED, _) => Body::empty(),
        (_, Cow::Borrowed(bytes)) => Body::from(bytes),
        (_, Cow::Owned(bytes)) => Body::from(bytes),
    };
    let mime = mime_guess::from_path(target).first_or_octet_stream();

    Response::builder()
        .status(status)
        .header(header::CACHE_CONTROL, cache_control_for(target))
        .header(header::ETAG, &etag)
        .header(header::CONTENT_TYPE, mime.as_ref())
        .body(body)
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

/// `If-None-Match` may hold a list and weak validators (proxies such as
/// Cloudflare weaken ETags when they recompress), so compare weakly per RFC 9110.
fn etag_matches(headers: &HeaderMap, etag: &str) -> bool {
    headers
        .get_all(header::IF_NONE_MATCH)
        .iter()
        .filter_map(|value: &HeaderValue| value.to_str().ok())
        .flat_map(|value| value.split(','))
        .map(|candidate| candidate.trim())
        .any(|candidate| candidate == "*" || candidate.trim_start_matches("W/") == etag)
}

#[derive(Debug, PartialEq, Eq)]
enum Fallback {
    Index,
    NotFound,
}

/// Client-side routes (`/cards/123`) get index.html; a missing file
/// (`/assets/old.js`) gets a real 404 so a stale service worker notices.
fn fallback_for(path: &str) -> Fallback {
    let last_segment = path.rsplit('/').next().unwrap_or_default();
    if last_segment.contains('.') {
        Fallback::NotFound
    } else {
        Fallback::Index
    }
}

fn cache_control_for(path: &str) -> &'static str {
    if path.starts_with(HASHED_ASSET_PREFIX) {
        IMMUTABLE
    } else {
        REVALIDATE
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use axum::body::to_bytes;

    use super::*;

    const INDEX_HTML: &[u8] = b"<!doctype html><title>Tessera</title>";
    const APP_JS: &[u8] = b"console.log('app')";

    struct MapSource(HashMap<&'static str, (&'static [u8], u8)>);

    impl AssetSource for MapSource {
        fn get(&self, path: &str) -> Option<Asset> {
            self.0.get(path).map(|(data, seed)| Asset {
                data: Cow::Borrowed(data),
                sha256: [*seed; 32],
            })
        }
    }

    fn built_frontend() -> MapSource {
        MapSource(HashMap::from([
            (INDEX, (INDEX_HTML, 1)),
            ("assets/index-abc123.js", (APP_JS, 2)),
            ("sw.js", (APP_JS, 3)),
        ]))
    }

    fn get(source: &impl AssetSource, path: &str) -> Response {
        respond(source, &Method::GET, path, &HeaderMap::new())
    }

    fn get_if_none_match(source: &impl AssetSource, path: &str, value: &str) -> Response {
        let mut headers = HeaderMap::new();
        headers.insert(header::IF_NONE_MATCH, HeaderValue::from_str(value).unwrap());
        respond(source, &Method::GET, path, &headers)
    }

    async fn body(response: Response) -> Vec<u8> {
        to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec()
    }

    fn index_etag() -> String {
        format!("\"{}\"", "01".repeat(32))
    }

    #[tokio::test]
    async fn root_serves_index_with_revalidation_and_etag() {
        let response = get(&built_frontend(), "/");

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[header::CONTENT_TYPE], "text/html");
        assert_eq!(response.headers()[header::CACHE_CONTROL], REVALIDATE);
        assert_eq!(response.headers()[header::ETAG], index_etag().as_str());
        assert_eq!(body(response).await, INDEX_HTML);
    }

    #[tokio::test]
    async fn client_route_serves_index() {
        let response = get(&built_frontend(), "/cards/0193-abc");

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(body(response).await, INDEX_HTML);
    }

    #[test]
    fn hashed_asset_is_immutable_with_its_own_type() {
        let response = get(&built_frontend(), "/assets/index-abc123.js");

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[header::CACHE_CONTROL], IMMUTABLE);
        assert_eq!(response.headers()[header::CONTENT_TYPE], "text/javascript");
    }

    #[test]
    fn service_worker_is_revalidated() {
        let response = get(&built_frontend(), "/sw.js");
        assert_eq!(response.headers()[header::CACHE_CONTROL], REVALIDATE);
    }

    #[test]
    fn missing_file_is_404_not_index() {
        assert_eq!(
            get(&built_frontend(), "/assets/old.js").status(),
            StatusCode::NOT_FOUND
        );
    }

    #[tokio::test]
    async fn matching_etag_returns_304_without_body() {
        let response = get_if_none_match(&built_frontend(), "/", &index_etag());

        assert_eq!(response.status(), StatusCode::NOT_MODIFIED);
        assert!(body(response).await.is_empty());
    }

    #[test]
    fn weak_and_listed_etags_match() {
        let weak = format!("W/{}", index_etag());
        let listed = format!("\"other\", {}", index_etag());

        assert_eq!(
            get_if_none_match(&built_frontend(), "/", &weak).status(),
            StatusCode::NOT_MODIFIED
        );
        assert_eq!(
            get_if_none_match(&built_frontend(), "/", &listed).status(),
            StatusCode::NOT_MODIFIED
        );
        assert_eq!(
            get_if_none_match(&built_frontend(), "/", "*").status(),
            StatusCode::NOT_MODIFIED
        );
    }

    #[test]
    fn stale_etag_returns_full_response() {
        let response = get_if_none_match(&built_frontend(), "/", "\"stale\"");
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[test]
    fn unbuilt_frontend_is_503() {
        let empty = MapSource(HashMap::new());
        assert_eq!(get(&empty, "/").status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[test]
    fn api_paths_never_get_index() {
        for path in ["/api", "/api/", "/api/v2/unknown"] {
            let response = get(&built_frontend(), path);
            assert_eq!(response.status(), StatusCode::NOT_FOUND, "{path}");
            assert_eq!(
                response.headers()[header::CONTENT_TYPE],
                "application/json",
                "{path}"
            );
        }
    }

    #[test]
    fn writes_are_rejected() {
        let response = respond(
            &built_frontend(),
            &Method::POST,
            "/cards",
            &HeaderMap::new(),
        );
        assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
    }

    #[test]
    fn hex_encodes_lowercase() {
        assert_eq!(hex(&[0x00, 0xab, 0xff]), "00abff");
    }
}
