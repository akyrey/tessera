//! Kubernetes probes. `/healthz` = process alive, `/readyz` = database reachable.

use axum::extract::State;

use crate::db;
use crate::error::ApiError;
use crate::state::AppState;

pub async fn live() -> &'static str {
    "ok"
}

pub async fn ready(State(state): State<AppState>) -> Result<&'static str, ApiError> {
    db::ping(&state.db).await.map_err(|error| {
        tracing::warn!(%error, "readiness check failed");
        ApiError::Unavailable
    })?;
    Ok("ok")
}
