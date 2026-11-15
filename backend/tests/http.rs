//! Router-level integration tests against a temporary SQLite database.

use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use http_body_util::BodyExt;
use serde_json::Value;
use tempfile::TempDir;
use tessera::config::Config;
use tessera::{AppState, db, router};
use tower::ServiceExt;

struct TestApp {
    router: Router,
    pool: sqlx::SqlitePool,
    _dir: TempDir,
}

async fn spawn_app() -> TestApp {
    let dir = tempfile::tempdir().expect("temp dir");
    let database_url = format!("sqlite://{}", dir.path().join("test.db").display());
    let config = Config::from_lookup(|key| match key {
        "TESSERA_PUBLIC_URL" => Some("http://localhost:8080".into()),
        "TESSERA_DATABASE_URL" => Some(database_url.clone()),
        _ => None,
    })
    .expect("valid config");
    let pool = db::connect(&config.database_url).await.expect("database");
    let state = AppState {
        db: pool.clone(),
        config: Arc::new(config),
    };
    TestApp {
        router: router(state),
        pool,
        _dir: dir,
    }
}

async fn get(app: &TestApp, uri: &str) -> axum::response::Response {
    let request = Request::get(uri).body(Body::empty()).unwrap();
    app.router.clone().oneshot(request).await.unwrap()
}

async fn json(response: axum::response::Response) -> Value {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

#[tokio::test]
async fn liveness_probe_is_ok() {
    let app = spawn_app().await;
    let response = get(&app, "/healthz").await;
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn readiness_probe_is_ok_when_database_is_reachable() {
    let app = spawn_app().await;
    let response = get(&app, "/readyz").await;
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn readiness_probe_fails_when_database_is_closed() {
    let app = spawn_app().await;
    app.pool.close().await;

    let response = get(&app, "/readyz").await;

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(json(response).await["error"]["code"], "unavailable");
}

#[tokio::test]
async fn version_endpoint_reports_package_and_is_not_cached() {
    let app = spawn_app().await;
    let response = get(&app, "/api/version").await;

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    let body = json(response).await;
    assert_eq!(body["name"], "tessera");
    assert_eq!(body["version"], env!("CARGO_PKG_VERSION"));
}

#[tokio::test]
async fn unknown_api_route_returns_json_404() {
    let app = spawn_app().await;
    let response = get(&app, "/api/nope").await;

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    assert_eq!(json(response).await["error"]["code"], "not_found");
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
        let response = get(&app, uri).await;
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
        let response = get(&app, uri).await;
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
    let response = get(&app, "/api/").await;

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert_eq!(json(response).await["error"]["code"], "not_found");
}

#[tokio::test]
async fn missing_static_file_is_404_not_index() {
    let app = spawn_app().await;
    let response = get(&app, "/assets/does-not-exist.js").await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn static_fallback_rejects_writes() {
    let app = spawn_app().await;
    let request = Request::post("/cards").body(Body::empty()).unwrap();
    let response = app.router.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
}
