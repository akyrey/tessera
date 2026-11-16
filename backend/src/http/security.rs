//! Security and default caching headers applied to every response.

use axum::http::{HeaderName, HeaderValue, header};
use axum::response::Response;

/// `wasm-unsafe-eval` is needed by the zxing-wasm barcode scanner; `blob:` by
/// camera frames and generated barcode images. OpenStreetMap tiles (map view)
/// are the only third-party resource, and only as images.
pub const CONTENT_SECURITY_POLICY: &str = "default-src 'self'; \
script-src 'self' 'wasm-unsafe-eval'; \
style-src 'self'; \
img-src 'self' data: blob: https://tile.openstreetmap.org; \
connect-src 'self'; \
worker-src 'self' blob:; \
manifest-src 'self'; \
object-src 'none'; \
base-uri 'self'; \
form-action 'self'; \
frame-ancestors 'none'";

pub const PERMISSIONS_POLICY: &str =
    "camera=(self), geolocation=(self), microphone=(), payment=(), usb=()";

const HEADERS: [(HeaderName, &str); 7] = [
    // Default for API, probes and errors; static files set their own policy.
    (header::CACHE_CONTROL, "no-store"),
    (header::CONTENT_SECURITY_POLICY, CONTENT_SECURITY_POLICY),
    (
        header::STRICT_TRANSPORT_SECURITY,
        "max-age=31536000; includeSubDomains",
    ),
    (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
    (header::REFERRER_POLICY, "no-referrer"),
    (header::X_FRAME_OPTIONS, "DENY"),
    (
        HeaderName::from_static("permissions-policy"),
        PERMISSIONS_POLICY,
    ),
];

/// `axum::middleware::map_response` hook; keeps any value a handler already set.
pub async fn apply_headers(mut response: Response) -> Response {
    let headers = response.headers_mut();
    for (name, value) in HEADERS {
        headers
            .entry(name)
            .or_insert_with(|| HeaderValue::from_static(value));
    }
    response
}

#[cfg(test)]
mod tests {
    use super::CONTENT_SECURITY_POLICY;

    fn directive(name: &str) -> &'static str {
        CONTENT_SECURITY_POLICY
            .split(';')
            .map(str::trim)
            .find(|d| d.starts_with(name))
            .unwrap_or_default()
    }

    #[test]
    fn only_map_tiles_come_from_a_third_party() {
        assert_eq!(
            directive("img-src"),
            "img-src 'self' data: blob: https://tile.openstreetmap.org"
        );
        for name in ["default-src", "script-src", "style-src", "connect-src"] {
            assert!(!directive(name).contains("https:"), "{name}");
        }
        assert_eq!(directive("connect-src"), "connect-src 'self'");
    }
}
