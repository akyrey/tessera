//! Periodic cleanup of expired sessions, invites, ceremonies and limiter state.

use std::time::Duration;

use tokio::task::JoinHandle;

use crate::accounts::repo;
use crate::clock::now_unix;
use crate::state::AppState;

const INTERVAL: Duration = Duration::from_secs(15 * 60);

pub fn spawn(state: AppState) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(INTERVAL);
        loop {
            ticker.tick().await;
            if let Err(error) = sweep(&state, now_unix()).await {
                tracing::error!(%error, "cleanup failed");
            }
        }
    })
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Swept {
    pub sessions: u64,
    pub invites: u64,
}

pub async fn sweep(state: &AppState, now: i64) -> sqlx::Result<Swept> {
    state.registrations.purge_expired();
    state.logins.purge_expired();
    state.rate_limits.retain_recent();
    let swept = Swept {
        sessions: repo::delete_expired_sessions(&state.db, now).await?,
        invites: repo::delete_expired_invites(&state.db, now).await?,
    };
    if swept != Swept::default() {
        tracing::info!(
            sessions = swept.sessions,
            invites = swept.invites,
            "expired rows removed"
        );
    }
    Ok(swept)
}
