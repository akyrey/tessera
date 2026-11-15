//! Cookie-based sessions.
//!
//! The cookie holds a random token; the `sessions` table stores its SHA-256.
//! Over HTTPS the cookie uses the `__Host-` prefix (Secure, Path=/, no Domain),
//! so subdomains cannot set or override it.

use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::http::{HeaderMap, HeaderValue, header};
use sqlx::SqlitePool;
use uuid::Uuid;

use super::model::User;
use super::repo;
use crate::clock::{now_unix, secs};
use crate::config::Config;
use crate::error::ApiError;
use crate::state::AppState;
use crate::tokens::{self, SecretToken};

const SECURE_NAME: &str = "__Host-tessera_session";
/// Plain-HTTP localhost development: WebKit refuses Secure cookies there.
const INSECURE_NAME: &str = "tessera_session";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionCookie {
    name: &'static str,
    secure: bool,
    ttl_secs: i64,
}

impl SessionCookie {
    pub fn from_config(config: &Config) -> Self {
        let secure = config.public_url.scheme() == "https";
        Self {
            name: if secure { SECURE_NAME } else { INSECURE_NAME },
            secure,
            ttl_secs: secs(config.session_ttl),
        }
    }

    pub fn ttl_secs(&self) -> i64 {
        self.ttl_secs
    }

    pub fn set(&self, token: &str) -> HeaderValue {
        self.header(token, self.ttl_secs)
    }

    pub fn clear(&self) -> HeaderValue {
        self.header("", 0)
    }

    fn header(&self, value: &str, max_age: i64) -> HeaderValue {
        let secure = if self.secure { "; Secure" } else { "" };
        let cookie = format!(
            "{}={value}; Path=/; Max-Age={max_age}; HttpOnly; SameSite=Strict{secure}",
            self.name
        );
        // Name and token are ASCII by construction.
        HeaderValue::from_str(&cookie).unwrap_or_else(|_| HeaderValue::from_static(""))
    }

    /// The session token from the request's `Cookie` headers, if well-formed.
    pub fn read<'a>(&self, headers: &'a HeaderMap) -> Option<&'a str> {
        headers
            .get_all(header::COOKIE)
            .iter()
            .filter_map(|value| value.to_str().ok())
            .flat_map(|value| value.split(';'))
            .filter_map(|pair| pair.trim().split_once('='))
            .find(|(name, _)| *name == self.name)
            .map(|(_, value)| value)
            .filter(|value| tokens::looks_valid(value))
    }
}

pub async fn start_session(
    pool: &SqlitePool,
    user_id: Uuid,
    ttl_secs: i64,
    now: i64,
) -> Result<SecretToken, ApiError> {
    let token = SecretToken::generate();
    repo::insert_session(
        pool,
        &token.hash,
        user_id,
        now,
        now.saturating_add(ttl_secs),
    )
    .await?;
    Ok(token)
}

/// Signs the user out on every device.
pub async fn end_all_sessions(pool: &SqlitePool, user_id: Uuid) -> Result<(), ApiError> {
    repo::delete_sessions_of(pool, user_id, None).await?;
    Ok(())
}

pub async fn end_session(pool: &SqlitePool, token: &str) -> Result<(), ApiError> {
    repo::delete_session(pool, &tokens::hash(token)).await?;
    Ok(())
}

/// The signed-in user; rejects the request with 401 otherwise.
pub struct CurrentUser(pub User);

impl FromRequestParts<AppState> for CurrentUser {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let token = state
            .cookie
            .read(&parts.headers)
            .ok_or(ApiError::Unauthorized)?;
        repo::find_session_user(&state.db, &tokens::hash(token), now_unix())
            .await?
            .map(CurrentUser)
            .ok_or(ApiError::Unauthorized)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cookie(url: &str) -> SessionCookie {
        let config =
            Config::from_lookup(|key| (key == "TESSERA_PUBLIC_URL").then(|| url.to_owned()))
                .unwrap();
        SessionCookie::from_config(&config)
    }

    fn headers(cookie_header: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            HeaderValue::from_str(cookie_header).unwrap(),
        );
        headers
    }

    #[test]
    fn https_cookie_is_host_prefixed_and_secure() {
        let value = cookie("https://tessera.example.com").set("tok");
        let value = value.to_str().unwrap();

        assert!(
            value.starts_with("__Host-tessera_session=tok; Path=/;"),
            "{value}"
        );
        assert!(value.contains("HttpOnly"));
        assert!(value.contains("SameSite=Strict"));
        assert!(value.ends_with("; Secure"));
        assert!(value.contains(&format!("Max-Age={}", 30 * 24 * 60 * 60)));
    }

    #[test]
    fn localhost_cookie_is_not_secure() {
        let value = cookie("http://localhost:8080").set("tok");
        let value = value.to_str().unwrap();

        assert!(value.starts_with("tessera_session=tok;"));
        assert!(!value.contains("Secure"));
    }

    #[test]
    fn clearing_expires_the_cookie_immediately() {
        let value = cookie("https://tessera.example.com").clear();
        assert!(value.to_str().unwrap().contains("Max-Age=0"));
    }

    #[test]
    fn reads_the_session_among_other_cookies() {
        let token = SecretToken::generate().value;
        let jar = headers(&format!(
            "theme=dark; __Host-tessera_session={token}; other=1"
        ));

        assert_eq!(
            cookie("https://tessera.example.com").read(&jar),
            Some(token.as_str())
        );
    }

    #[test]
    fn ignores_missing_or_malformed_session_cookies() {
        let c = cookie("https://tessera.example.com");
        assert_eq!(c.read(&HeaderMap::new()), None);
        assert_eq!(c.read(&headers("__Host-tessera_session=short")), None);
        // The insecure name is not accepted when the secure one is expected.
        let token = SecretToken::generate().value;
        assert_eq!(c.read(&headers(&format!("tessera_session={token}"))), None);
    }
}
