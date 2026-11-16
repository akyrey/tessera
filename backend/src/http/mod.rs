//! HTTP router: probes, JSON API under `/api`, and the embedded PWA.

mod health;
mod meta;
pub mod origin;
pub mod rate_limit;
mod security;
mod static_files;

use axum::extract::DefaultBodyLimit;
use axum::routing::get;
use axum::{Router, middleware};
use tower_http::compression::CompressionLayer;
use tower_http::trace::TraceLayer;

use crate::error::ApiError;
use crate::state::AppState;
use crate::{accounts, cards};

pub use meta::AppInfo;

/// JSON bodies are small (WebAuthn credentials are a few KiB); photo uploads
/// raise their own limit.
const API_BODY_LIMIT: usize = 64 * 1024;

pub fn router(state: AppState) -> Router {
    // Layers run outermost-last: rate limit → Origin check → body limit → handler.
    let api = Router::new()
        .route("/version", get(meta::version))
        .merge(accounts::routes(&state))
        .merge(cards::routes())
        .fallback(api_not_found)
        .layer(DefaultBodyLimit::max(API_BODY_LIMIT))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            origin::require_same_origin,
        ))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            rate_limit::limit_api,
        ));

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
