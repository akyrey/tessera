//! HTTP router: probes, JSON API under `/api`, and the embedded PWA.

mod health;
mod meta;
mod security;
mod static_files;

use std::sync::Arc;

use axum::routing::get;
use axum::{Router, middleware};
use sqlx::SqlitePool;
use tower_http::compression::CompressionLayer;
use tower_http::trace::TraceLayer;

use crate::config::Config;
use crate::error::ApiError;

pub use meta::AppInfo;

#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
    pub config: Arc<Config>,
}

pub fn router(state: AppState) -> Router {
    let api = Router::new()
        .route("/version", get(meta::version))
        .fallback(api_not_found);

    Router::new()
        .route("/healthz", get(health::live))
        .route("/readyz", get(health::ready))
        .nest("/api", api)
        .fallback(static_files::serve)
        .layer(middleware::map_response(security::apply_headers))
        .layer(CompressionLayer::new())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

async fn api_not_found() -> ApiError {
    ApiError::NotFound
}
