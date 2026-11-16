//! Background job keeping store locations and logos of linked brands up to date.

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
/// Logos rarely change; look again once a month.
const LOGO_REFRESH_SECS: i64 = 30 * 24 * 60 * 60;

pub fn spawn(state: AppState) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(TICK);
        loop {
            ticker.tick().await;
            if let Err(error) = refresh_due_logos(&state, now_unix()).await {
                tracing::error!(%error, "brand logo refresh failed");
            }
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

/// Looks up, one by one, the logo of every brand never or long ago checked.
pub async fn refresh_due_logos(state: &AppState, now: i64) -> sqlx::Result<usize> {
    let due = repo::logos_due(&state.db, now - LOGO_REFRESH_SECS, now - ERROR_BACKOFF_SECS).await?;
    let mut stored = 0;
    for (brand_id, qid) in due {
        if let Ok(true) =
            service::refresh_logo(&state.db, &state.upstream, brand_id, &qid, now_unix()).await
        {
            stored += 1;
        }
    }
    Ok(stored)
}

/// Fetches the logo and shops of a brand that has never been fetched, right
/// after a card starts using it. At most one such fetch per brand is queued at a time.
pub async fn fetch_if_new(state: &AppState, brand_id: Uuid) {
    let Ok(Some((qid, last_synced))) = repo::sync_state(&state.db, brand_id).await else {
        return;
    };
    let needs_logo = !repo::logo_checked(&state.db, brand_id)
        .await
        .unwrap_or(true);
    if (last_synced.is_some() && !needs_logo) || !state.upstream.claim_first_fetch(brand_id) {
        return;
    }
    let state = state.clone();
    tokio::spawn(async move {
        // The logo first: it takes a second, the shops can take a minute.
        if needs_logo {
            let _ =
                service::refresh_logo(&state.db, &state.upstream, brand_id, &qid, now_unix()).await;
        }
        if last_synced.is_none() {
            let _ =
                service::refresh_locations(&state.db, &state.upstream, brand_id, &qid, now_unix())
                    .await;
        }
        state.upstream.release_first_fetch(brand_id);
    });
}
