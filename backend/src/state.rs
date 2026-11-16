//! Shared application state handed to every request handler.

use std::sync::Arc;

use sqlx::SqlitePool;
use webauthn_rs::prelude::{DiscoverableAuthentication, Webauthn, WebauthnError};

use crate::accounts::RegistrationCeremony;
use crate::accounts::ceremonies::{
    CEREMONY_TTL, CeremonyStore, MAX_PENDING, MAX_PENDING_PER_CLIENT,
};
use crate::accounts::passkeys::build_webauthn;
use crate::accounts::sessions::SessionCookie;
use crate::brands::upstream::Upstream;
use crate::config::Config;
use crate::http::rate_limit::RateLimits;

#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
    pub config: Arc<Config>,
    pub webauthn: Arc<Webauthn>,
    pub cookie: SessionCookie,
    pub registrations: Arc<CeremonyStore<RegistrationCeremony>>,
    pub logins: Arc<CeremonyStore<DiscoverableAuthentication>>,
    pub rate_limits: Arc<RateLimits>,
    /// Wikidata and Overpass client.
    pub upstream: Arc<Upstream>,
}

#[derive(Debug, thiserror::Error)]
pub enum StateError {
    #[error("WebAuthn setup failed: {0}")]
    Webauthn(#[from] WebauthnError),
    #[error("HTTP client setup failed: {0}")]
    Http(#[from] reqwest::Error),
}

impl AppState {
    pub fn new(config: Config, db: SqlitePool) -> Result<Self, StateError> {
        Ok(Self {
            upstream: Arc::new(Upstream::new(&config)?),
            webauthn: Arc::new(build_webauthn(&config)?),
            cookie: SessionCookie::from_config(&config),
            registrations: Arc::new(ceremony_store()),
            logins: Arc::new(ceremony_store()),
            rate_limits: Arc::new(RateLimits::new()),
            config: Arc::new(config),
            db,
        })
    }
}

fn ceremony_store<T>() -> CeremonyStore<T> {
    CeremonyStore::new(CEREMONY_TTL, MAX_PENDING, MAX_PENDING_PER_CLIENT)
}
