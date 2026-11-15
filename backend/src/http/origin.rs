//! CSRF defence: state-changing API requests must come from our own origin.
//!
//! Complements `SameSite=Strict` session cookies. Browsers always send
//! `Origin` on cross-origin and on POST/PUT/DELETE fetches; when it is absent
//! we fall back to the Fetch Metadata `Sec-Fetch-Site` header.

use axum::extract::{Request, State};
use axum::http::{HeaderMap, Method};
use axum::middleware::Next;
use axum::response::Response;

use crate::error::ApiError;
use crate::state::AppState;

pub async fn require_same_origin(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    if is_safe(request.method()) || is_same_origin(request.headers(), &state.config.public_origin())
    {
        Ok(next.run(request).await)
    } else {
        Err(ApiError::BadOrigin)
    }
}

fn is_safe(method: &Method) -> bool {
    matches!(*method, Method::GET | Method::HEAD | Method::OPTIONS)
}

fn is_same_origin(headers: &HeaderMap, expected: &str) -> bool {
    match headers.get("origin") {
        Some(origin) => origin.as_bytes() == expected.as_bytes(),
        None => headers
            .get("sec-fetch-site")
            .is_some_and(|site| site.as_bytes() == b"same-origin"),
    }
}

#[cfg(test)]
mod tests {
    use axum::http::HeaderValue;

    use super::*;

    const ORIGIN: &str = "https://tessera.example.com";

    fn headers(pairs: &[(&'static str, &'static str)]) -> HeaderMap {
        let mut map = HeaderMap::new();
        for (name, value) in pairs {
            map.insert(*name, HeaderValue::from_static(value));
        }
        map
    }

    #[test]
    fn accepts_matching_origin() {
        assert!(is_same_origin(&headers(&[("origin", ORIGIN)]), ORIGIN));
    }

    #[test]
    fn rejects_foreign_or_null_origin() {
        assert!(!is_same_origin(
            &headers(&[("origin", "https://evil.example")]),
            ORIGIN
        ));
        assert!(!is_same_origin(&headers(&[("origin", "null")]), ORIGIN));
        // A matching Sec-Fetch-Site cannot rescue a wrong Origin.
        assert!(!is_same_origin(
            &headers(&[
                ("origin", "https://evil.example"),
                ("sec-fetch-site", "same-origin")
            ]),
            ORIGIN
        ));
    }

    #[test]
    fn falls_back_to_fetch_metadata_without_origin() {
        assert!(is_same_origin(
            &headers(&[("sec-fetch-site", "same-origin")]),
            ORIGIN
        ));
        assert!(!is_same_origin(
            &headers(&[("sec-fetch-site", "cross-site")]),
            ORIGIN
        ));
        assert!(!is_same_origin(&HeaderMap::new(), ORIGIN));
    }

    #[test]
    fn only_reads_are_exempt() {
        assert!(is_safe(&Method::GET));
        assert!(is_safe(&Method::HEAD));
        assert!(!is_safe(&Method::POST));
        assert!(!is_safe(&Method::DELETE));
    }
}
