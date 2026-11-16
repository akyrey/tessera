//! Runtime configuration loaded from `TESSERA_*` environment variables.
//!
//! Every value is validated at startup so a misconfigured deployment fails fast
//! with a message naming the offending variable.

use std::net::SocketAddr;
use std::time::Duration;

use url::Url;

use crate::tokens;

const PREFIX: &str = "TESSERA_";

const DEFAULT_RP_NAME: &str = "Tessera";
const DEFAULT_DATABASE_URL: &str = "sqlite://tessera.db";
const DEFAULT_LISTEN_ADDR: &str = "0.0.0.0:8080";
const DEFAULT_REGION: &str = "IT";
const DEFAULT_NEARBY_RADIUS_M: u32 = 200;
const MAX_NEARBY_RADIUS_M: u32 = 5_000;
const DEFAULT_STORE_REFRESH_INTERVAL: Duration = Duration::from_secs(7 * 24 * 60 * 60);
const DEFAULT_OVERPASS_URL: &str = "https://overpass-api.de/api/interpreter";
const DEFAULT_WIKIDATA_URL: &str = "https://www.wikidata.org/w/api.php";
const DEFAULT_SESSION_TTL: Duration = Duration::from_secs(30 * 24 * 60 * 60);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// Exact origin the browser sees, e.g. `https://tessera.example.com`.
    pub public_url: Url,
    /// WebAuthn relying-party ID: the public host or one of its parent domains.
    pub webauthn_rp_id: String,
    pub webauthn_rp_name: String,
    pub database_url: String,
    pub listen_addr: SocketAddr,
    /// ISO 3166-1 alpha-2 country code used for store-location lookups.
    pub region_iso3166: String,
    pub nearby_radius_m: u32,
    pub store_refresh_interval: Duration,
    pub session_ttl: Duration,
    /// Fixed token for the first-admin invite link; random when unset.
    pub bootstrap_token: Option<Secret>,
    /// OpenStreetMap Overpass API endpoint (store locations).
    pub overpass_url: Url,
    /// Wikidata `api.php` endpoint (brand search).
    pub wikidata_url: Url,
}

/// A configuration value that must never appear in logs or `Debug` output.
#[derive(Clone, PartialEq, Eq)]
pub struct Secret(String);

impl Secret {
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Secret([redacted])")
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ConfigError {
    #[error("{PREFIX}{0} is required")]
    Missing(&'static str),
    #[error("{PREFIX}{name} is invalid: {reason}")]
    Invalid { name: &'static str, reason: String },
}

impl ConfigError {
    fn invalid(name: &'static str, reason: impl Into<String>) -> Self {
        Self::Invalid {
            name,
            reason: reason.into(),
        }
    }
}

impl Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        Self::from_lookup(|key| std::env::var(key).ok())
    }

    /// Builds the config from any key lookup; `from_env` uses the process environment.
    pub fn from_lookup(lookup: impl Fn(&str) -> Option<String>) -> Result<Self, ConfigError> {
        let get = |name: &str| {
            lookup(&format!("{PREFIX}{name}"))
                .map(|value| value.trim().to_owned())
                .filter(|value| !value.is_empty())
        };

        let public_url = parse_public_url(get("PUBLIC_URL"))?;
        let host = public_url
            .host_str()
            .ok_or_else(|| ConfigError::invalid("PUBLIC_URL", "must contain a host"))?
            .to_owned();
        let webauthn_rp_id = parse_rp_id(get("WEBAUTHN_RP_ID"), &host)?;

        Ok(Self {
            public_url,
            webauthn_rp_id,
            webauthn_rp_name: get("WEBAUTHN_RP_NAME").unwrap_or_else(|| DEFAULT_RP_NAME.into()),
            database_url: get("DATABASE_URL").unwrap_or_else(|| DEFAULT_DATABASE_URL.into()),
            listen_addr: parse_listen_addr(get("LISTEN_ADDR"))?,
            region_iso3166: parse_region(get("REGION_ISO3166"))?,
            nearby_radius_m: parse_radius(get("NEARBY_RADIUS_M"))?,
            store_refresh_interval: parse_duration(
                "STORE_REFRESH_INTERVAL",
                get("STORE_REFRESH_INTERVAL"),
                DEFAULT_STORE_REFRESH_INTERVAL,
            )?,
            session_ttl: parse_duration("SESSION_TTL", get("SESSION_TTL"), DEFAULT_SESSION_TTL)?,
            bootstrap_token: parse_bootstrap_token(get("BOOTSTRAP_TOKEN"))?,
            overpass_url: parse_upstream_url(
                "OVERPASS_URL",
                get("OVERPASS_URL"),
                DEFAULT_OVERPASS_URL,
            )?,
            wikidata_url: parse_upstream_url(
                "WIKIDATA_URL",
                get("WIKIDATA_URL"),
                DEFAULT_WIKIDATA_URL,
            )?,
        })
    }

    /// Origin string (`scheme://host[:port]`) used for Origin-header checks.
    pub fn public_origin(&self) -> String {
        self.public_url.origin().ascii_serialization()
    }
}

/// An external API endpoint: https, or http only for a local test double.
fn parse_upstream_url(
    name: &'static str,
    raw: Option<String>,
    default: &str,
) -> Result<Url, ConfigError> {
    let url = Url::parse(raw.as_deref().unwrap_or(default))
        .map_err(|e| ConfigError::invalid(name, e.to_string()))?;
    let is_local = matches!(url.host_str(), Some("localhost" | "127.0.0.1"));
    match url.scheme() {
        "https" => Ok(url),
        "http" if is_local => Ok(url),
        _ => Err(ConfigError::invalid(name, "must use https")),
    }
}

fn parse_public_url(raw: Option<String>) -> Result<Url, ConfigError> {
    const NAME: &str = "PUBLIC_URL";
    let raw = raw.ok_or(ConfigError::Missing(NAME))?;
    let url = Url::parse(&raw).map_err(|e| ConfigError::invalid(NAME, e.to_string()))?;

    let is_local = matches!(url.host_str(), Some("localhost" | "127.0.0.1"));
    match url.scheme() {
        "https" => {}
        "http" if is_local => {}
        _ => {
            return Err(ConfigError::invalid(
                NAME,
                "must use https (http only for localhost)",
            ));
        }
    }
    if url.path() != "/" || url.query().is_some() || url.fragment().is_some() {
        return Err(ConfigError::invalid(
            NAME,
            "must be an origin without path, query or fragment",
        ));
    }
    Ok(url)
}

/// The RP ID must equal the public host or be a parent domain of it (WebAuthn rule).
fn parse_rp_id(raw: Option<String>, host: &str) -> Result<String, ConfigError> {
    let Some(rp_id) = raw.map(|value| value.to_ascii_lowercase()) else {
        return Ok(host.to_owned());
    };
    let is_parent_domain = host
        .strip_suffix(rp_id.as_str())
        .is_some_and(|rest| rest.ends_with('.'));
    if rp_id == host || is_parent_domain {
        Ok(rp_id)
    } else {
        Err(ConfigError::invalid(
            "WEBAUTHN_RP_ID",
            format!("'{rp_id}' must equal or be a parent domain of '{host}'"),
        ))
    }
}

fn parse_listen_addr(raw: Option<String>) -> Result<SocketAddr, ConfigError> {
    raw.as_deref()
        .unwrap_or(DEFAULT_LISTEN_ADDR)
        .parse()
        .map_err(|e: std::net::AddrParseError| ConfigError::invalid("LISTEN_ADDR", e.to_string()))
}

fn parse_region(raw: Option<String>) -> Result<String, ConfigError> {
    let region = raw
        .unwrap_or_else(|| DEFAULT_REGION.into())
        .to_ascii_uppercase();
    if region.len() == 2 && region.chars().all(|c| c.is_ascii_alphabetic()) {
        Ok(region)
    } else {
        Err(ConfigError::invalid(
            "REGION_ISO3166",
            "must be a two-letter ISO 3166-1 code",
        ))
    }
}

fn parse_radius(raw: Option<String>) -> Result<u32, ConfigError> {
    const NAME: &str = "NEARBY_RADIUS_M";
    let Some(raw) = raw else {
        return Ok(DEFAULT_NEARBY_RADIUS_M);
    };
    match raw.parse::<u32>() {
        Ok(radius) if (1..=MAX_NEARBY_RADIUS_M).contains(&radius) => Ok(radius),
        _ => Err(ConfigError::invalid(
            NAME,
            format!("must be an integer between 1 and {MAX_NEARBY_RADIUS_M}"),
        )),
    }
}

fn parse_bootstrap_token(raw: Option<String>) -> Result<Option<Secret>, ConfigError> {
    match raw {
        None => Ok(None),
        Some(token) if tokens::looks_valid(&token) && tokens::is_strong(&token) => {
            Ok(Some(Secret(token)))
        }
        Some(_) => Err(ConfigError::invalid(
            "BOOTSTRAP_TOKEN",
            format!(
                "must be {} base64url characters of random data (32 bytes), e.g. \
                 `openssl rand -base64 32 | tr '+/' '-_' | tr -d '='`",
                tokens::ENCODED_LEN
            ),
        )),
    }
}

fn parse_duration(
    name: &'static str,
    raw: Option<String>,
    default: Duration,
) -> Result<Duration, ConfigError> {
    let Some(raw) = raw else {
        return Ok(default);
    };
    match humantime::parse_duration(&raw) {
        Ok(duration) if !duration.is_zero() => Ok(duration),
        Ok(_) => Err(ConfigError::invalid(name, "must be greater than zero")),
        Err(e) => Err(ConfigError::invalid(name, e.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn load(vars: &[(&str, &str)]) -> Result<Config, ConfigError> {
        let map: HashMap<String, String> = vars
            .iter()
            .map(|(k, v)| (format!("{PREFIX}{k}"), (*v).to_owned()))
            .collect();
        Config::from_lookup(|key| map.get(key).cloned())
    }

    fn load_with_url(extra: &[(&str, &str)]) -> Result<Config, ConfigError> {
        let mut vars = vec![("PUBLIC_URL", "https://tessera.example.com")];
        vars.extend_from_slice(extra);
        load(&vars)
    }

    #[test]
    fn requires_public_url() {
        assert_eq!(load(&[]), Err(ConfigError::Missing("PUBLIC_URL")));
    }

    #[test]
    fn treats_blank_values_as_missing() {
        assert_eq!(
            load(&[("PUBLIC_URL", "   ")]),
            Err(ConfigError::Missing("PUBLIC_URL"))
        );
    }

    #[test]
    fn applies_defaults_when_only_public_url_is_set() {
        let config = load_with_url(&[]).unwrap();

        assert_eq!(config.webauthn_rp_id, "tessera.example.com");
        assert_eq!(config.webauthn_rp_name, "Tessera");
        assert_eq!(config.listen_addr, "0.0.0.0:8080".parse().unwrap());
        assert_eq!(config.region_iso3166, "IT");
        assert_eq!(config.nearby_radius_m, 200);
        assert_eq!(
            config.store_refresh_interval,
            DEFAULT_STORE_REFRESH_INTERVAL
        );
        assert_eq!(config.session_ttl, DEFAULT_SESSION_TTL);
        assert_eq!(config.public_origin(), "https://tessera.example.com");
        assert_eq!(config.overpass_url.as_str(), DEFAULT_OVERPASS_URL);
        assert_eq!(config.wikidata_url.as_str(), DEFAULT_WIKIDATA_URL);
    }

    #[test]
    fn upstream_urls_must_be_https_except_local_test_doubles() {
        let local = load_with_url(&[("OVERPASS_URL", "http://127.0.0.1:9000/api")]).unwrap();
        assert_eq!(local.overpass_url.as_str(), "http://127.0.0.1:9000/api");

        for (name, value) in [
            ("OVERPASS_URL", "http://overpass.example/api"),
            ("WIKIDATA_URL", "ftp://wikidata.example"),
            ("WIKIDATA_URL", "not a url"),
        ] {
            assert!(
                matches!(
                    load_with_url(&[(name, value)]),
                    Err(ConfigError::Invalid { .. })
                ),
                "{name}={value}"
            );
        }
    }

    #[test]
    fn rejects_plain_http_for_public_hosts() {
        let err = load(&[("PUBLIC_URL", "http://tessera.example.com")]).unwrap_err();
        assert!(matches!(
            err,
            ConfigError::Invalid {
                name: "PUBLIC_URL",
                ..
            }
        ));
    }

    #[test]
    fn allows_plain_http_for_localhost() {
        let config = load(&[("PUBLIC_URL", "http://localhost:5173")]).unwrap();
        assert_eq!(config.public_origin(), "http://localhost:5173");
        assert_eq!(config.webauthn_rp_id, "localhost");
    }

    #[test]
    fn rejects_public_url_with_path() {
        let err = load(&[("PUBLIC_URL", "https://example.com/app")]).unwrap_err();
        assert!(matches!(
            err,
            ConfigError::Invalid {
                name: "PUBLIC_URL",
                ..
            }
        ));
    }

    #[test]
    fn accepts_parent_domain_as_rp_id() {
        let config = load_with_url(&[("WEBAUTHN_RP_ID", "Example.com")]).unwrap();
        assert_eq!(config.webauthn_rp_id, "example.com");
    }

    #[test]
    fn rejects_rp_id_that_only_shares_a_suffix() {
        let err = load(&[
            ("PUBLIC_URL", "https://tessera.badexample.com"),
            ("WEBAUTHN_RP_ID", "example.com"),
        ])
        .unwrap_err();
        assert!(matches!(
            err,
            ConfigError::Invalid {
                name: "WEBAUTHN_RP_ID",
                ..
            }
        ));
    }

    #[test]
    fn rejects_unrelated_rp_id() {
        let err = load_with_url(&[("WEBAUTHN_RP_ID", "other.org")]).unwrap_err();
        assert!(matches!(
            err,
            ConfigError::Invalid {
                name: "WEBAUTHN_RP_ID",
                ..
            }
        ));
    }

    #[test]
    fn normalizes_and_validates_region() {
        assert_eq!(
            load_with_url(&[("REGION_ISO3166", "fr")])
                .unwrap()
                .region_iso3166,
            "FR"
        );
        assert!(load_with_url(&[("REGION_ISO3166", "ITA")]).is_err());
        assert!(load_with_url(&[("REGION_ISO3166", "1T")]).is_err());
    }

    #[test]
    fn validates_nearby_radius_bounds() {
        assert_eq!(
            load_with_url(&[("NEARBY_RADIUS_M", "350")])
                .unwrap()
                .nearby_radius_m,
            350
        );
        assert!(load_with_url(&[("NEARBY_RADIUS_M", "0")]).is_err());
        assert!(load_with_url(&[("NEARBY_RADIUS_M", "5001")]).is_err());
        assert!(load_with_url(&[("NEARBY_RADIUS_M", "far")]).is_err());
    }

    #[test]
    fn parses_human_durations() {
        let config =
            load_with_url(&[("SESSION_TTL", "12h"), ("STORE_REFRESH_INTERVAL", "1d")]).unwrap();
        assert_eq!(config.session_ttl, Duration::from_secs(12 * 3600));
        assert_eq!(
            config.store_refresh_interval,
            Duration::from_secs(24 * 3600)
        );
    }

    #[test]
    fn rejects_zero_or_malformed_durations() {
        assert!(load_with_url(&[("SESSION_TTL", "0s")]).is_err());
        assert!(load_with_url(&[("SESSION_TTL", "soon")]).is_err());
    }

    #[test]
    fn rejects_malformed_listen_addr() {
        let err = load_with_url(&[("LISTEN_ADDR", "8080")]).unwrap_err();
        assert!(matches!(
            err,
            ConfigError::Invalid {
                name: "LISTEN_ADDR",
                ..
            }
        ));
    }

    #[test]
    fn bootstrap_token_is_optional_validated_and_redacted() {
        assert_eq!(load_with_url(&[]).unwrap().bootstrap_token, None);
        assert!(load_with_url(&[("BOOTSTRAP_TOKEN", "too-short")]).is_err());

        assert!(load_with_url(&[("BOOTSTRAP_TOKEN", &"x".repeat(tokens::ENCODED_LEN))]).is_err());

        let token = tokens::SecretToken::generate().value;
        let config = load_with_url(&[("BOOTSTRAP_TOKEN", &token)]).unwrap();
        assert_eq!(
            config.bootstrap_token.as_ref().map(Secret::expose),
            Some(token.as_str())
        );
        assert!(!format!("{config:?}").contains(&token));
    }

    #[test]
    fn error_messages_name_the_full_variable() {
        assert_eq!(
            ConfigError::Missing("PUBLIC_URL").to_string(),
            "TESSERA_PUBLIC_URL is required"
        );
    }
}
