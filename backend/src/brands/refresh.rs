//! Background job keeping store locations of linked brands up to date.

use std::time::Duration;

use tokio::task::JoinHandle;
use uuid::Uuid;

use super::{repo, service};
use crate::clock::{now_unix, secs};
use crate::state::AppState;

/// How often the job looks for stale brands.
const TICK: Duration = Duration::from_secs(10 * 60);
/// Wait this long after a failed Overpass query before retrying a brand.
const ERROR_BACKOFF_SECS: i64 = 6 * 60 * 60;

pub fn spawn(state: AppState) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(TICK);
        loop {
            ticker.tick().await;
            if let Err(error) = refresh_due(&state, now_unix()).await {
                tracing::error!(%error, "store location refresh failed");
            }
        }
    })
}

/// Refreshes, one by one, every brand whose locations are missing or stale.
pub async fn refresh_due(state: &AppState, now: i64) -> sqlx::Result<usize> {
    let interval = secs(state.config.store_refresh_interval);
    let due = repo::due_for_refresh(&state.db, now - interval, now - ERROR_BACKOFF_SECS).await?;
    let mut refreshed = 0;
    for (brand_id, qid) in due {
        if service::refresh_locations(&state.db, &state.upstream, brand_id, &qid, now_unix())
            .await
            .is_ok()
        {
            refreshed += 1;
        }
    }
    Ok(refreshed)
}

/// Fetches a newly linked brand's shops right away, without blocking the request.
pub fn refresh_soon(state: &AppState, brand_id: Uuid, qid: String) {
    let state = state.clone();
    tokio::spawn(async move {
        let _ = service::refresh_locations(&state.db, &state.upstream, brand_id, &qid, now_unix())
            .await;
    });
}
