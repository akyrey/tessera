//! Router-level integration tests against a temporary SQLite database.

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use common::{error_code, json, spawn_app};

#[tokio::test]
async fn liveness_probe_is_ok() {
    let app = spawn_app().await;
    let response = app.get("/healthz", None).await;
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn readiness_probe_is_ok_when_database_is_reachable() {
    let app = spawn_app().await;
    let response = app.get("/readyz", None).await;
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn readiness_probe_fails_when_database_is_closed() {
    let app = spawn_app().await;
    app.state.db.close().await;

    let response = app.get("/readyz", None).await;

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(error_code(response).await, "unavailable");
}

#[tokio::test]
async fn version_endpoint_reports_package_and_is_not_cached() {
    let app = spawn_app().await;
    let response = app.get("/api/version", None).await;

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    let body = json(response).await;
    assert_eq!(body["name"], "tessera");
    assert_eq!(body["version"], env!("CARGO_PKG_VERSION"));
}

#[tokio::test]
async fn unknown_api_route_returns_json_404() {
    let app = spawn_app().await;
    let response = app.get("/api/nope", None).await;

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    assert_eq!(error_code(response).await, "not_found");
}

#[tokio::test]
async fn every_response_carries_security_headers() {
    let app = spawn_app().await;
    for uri in [
        "/healthz",
        "/api/version",
        "/api/nope",
        "/some/client/route",
    ] {
        let response = app.get(uri, None).await;
        let headers = response.headers();
        for name in [
            "content-security-policy",
            "strict-transport-security",
            "x-content-type-options",
            "referrer-policy",
            "x-frame-options",
            "permissions-policy",
        ] {
            assert!(headers.contains_key(name), "{uri} is missing {name}");
        }
    }
}

#[tokio::test]
async fn probes_and_errors_are_never_cached() {
    let app = spawn_app().await;
    for uri in [
        "/healthz",
        "/readyz",
        "/api/nope",
        "/assets/does-not-exist.js",
    ] {
        let response = app.get(uri, None).await;
        assert_eq!(
            response.headers()[header::CACHE_CONTROL],
            "no-store",
            "{uri}"
        );
    }
}

#[tokio::test]
async fn api_trailing_slash_is_json_404_not_the_spa() {
    let app = spawn_app().await;
    let response = app.get("/api/", None).await;

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert_eq!(error_code(response).await, "not_found");
}

#[tokio::test]
async fn missing_static_file_is_404_not_index() {
    let app = spawn_app().await;
    let response = app.get("/assets/does-not-exist.js", None).await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn static_fallback_rejects_writes() {
    let app = spawn_app().await;
    let request = Request::post("/cards").body(Body::empty()).unwrap();
    let response = app.send(request).await;
    assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
}
