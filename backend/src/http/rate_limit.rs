//! Per-client-IP rate limiting (GCRA via `governor`).
//!
//! Behind Cloudflare Tunnel every TCP peer is the `cloudflared` pod, so the
//! client IP comes from `CF-Connecting-IP`. That header is trustworthy only
//! because the NetworkPolicy lets nothing but `cloudflared` reach the pod.
//!
//! IPv6 clients are bucketed by their /64 prefix: a single subscriber usually
//! controls a whole /64, so per-address keys would be trivial to rotate.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::num::NonZeroU32;

use axum::extract::{ConnectInfo, FromRequestParts, Request, State};
use axum::http::request::Parts;
use axum::http::{Extensions, HeaderMap};
use axum::middleware::Next;
use axum::response::Response;
use governor::{DefaultKeyedRateLimiter, Quota, RateLimiter};

use crate::error::ApiError;
use crate::state::AppState;

const CLIENT_IP_HEADER: &str = "cf-connecting-ip";
/// Sign-in, registration and invite lookups: enough for a few attempts per minute.
const AUTH_PER_MINUTE: NonZeroU32 = NonZeroU32::new(20).unwrap();
/// Everything under `/api`.
const API_PER_MINUTE: NonZeroU32 = NonZeroU32::new(600).unwrap();
const API_BURST: NonZeroU32 = NonZeroU32::new(120).unwrap();
/// Requests that call Wikidata for the user (brand search while typing).
const UPSTREAM_PER_MINUTE: NonZeroU32 = NonZeroU32::new(30).unwrap();

type Limiter = DefaultKeyedRateLimiter<IpAddr>;

pub struct RateLimits {
    auth: Limiter,
    api: Limiter,
    upstream: Limiter,
}

impl RateLimits {
    pub fn new() -> Self {
        Self {
            auth: RateLimiter::keyed(Quota::per_minute(AUTH_PER_MINUTE)),
            api: RateLimiter::keyed(Quota::per_minute(API_PER_MINUTE).allow_burst(API_BURST)),
            upstream: RateLimiter::keyed(Quota::per_minute(UPSTREAM_PER_MINUTE)),
        }
    }

    /// Drops state for clients that have been quiet long enough to be "full" again.
    pub fn retain_recent(&self) {
        self.auth.retain_recent();
        self.api.retain_recent();
        self.upstream.retain_recent();
    }
}

impl Default for RateLimits {
    fn default() -> Self {
        Self::new()
    }
}

pub async fn limit_api(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    check(&state.rate_limits.api, &request)?;
    Ok(next.run(request).await)
}

pub async fn limit_upstream(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    check(&state.rate_limits.upstream, &request)?;
    Ok(next.run(request).await)
}

pub async fn limit_auth(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    check(&state.rate_limits.auth, &request)?;
    Ok(next.run(request).await)
}

fn check(limiter: &Limiter, request: &Request) -> Result<(), ApiError> {
    let client = client_key(request.headers(), request.extensions());
    limiter.check_key(&client).map_err(|_| {
        tracing::warn!(%client, path = request.uri().path(), "rate limit exceeded");
        ApiError::RateLimited
    })
}

/// The caller's rate-limit key (see [`client_key`]), for handlers that need it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ClientIp(pub IpAddr);

impl<S: Send + Sync> FromRequestParts<S> for ClientIp {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        Ok(Self(client_key(&parts.headers, &parts.extensions)))
    }
}

/// Client address from `CF-Connecting-IP`, else the TCP peer, normalised so
/// one client maps to one key: IPv4-mapped IPv6 becomes IPv4, IPv6 becomes its /64.
pub fn client_key(headers: &HeaderMap, extensions: &Extensions) -> IpAddr {
    let ip = headers
        .get(CLIENT_IP_HEADER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.trim().parse().ok())
        .or_else(|| {
            extensions
                .get::<ConnectInfo<SocketAddr>>()
                .map(|ConnectInfo(addr)| addr.ip())
        })
        .unwrap_or(IpAddr::V4(Ipv4Addr::UNSPECIFIED));
    match ip {
        IpAddr::V6(v6) => match v6.to_ipv4_mapped() {
            Some(v4) => IpAddr::V4(v4),
            None => IpAddr::V6(Ipv6Addr::from_bits(v6.to_bits() & IPV6_PREFIX_MASK)),
        },
        v4 => v4,
    }
}

/// Keeps the first 64 bits (network prefix) of an IPv6 address.
const IPV6_PREFIX_MASK: u128 = !0u128 << 64;

#[cfg(test)]
mod tests {
    use axum::body::Body;

    use super::*;

    fn client_ip(request: &Request) -> IpAddr {
        client_key(request.headers(), request.extensions())
    }

    fn request(header: Option<&str>, peer: Option<&str>) -> Request {
        let mut builder = Request::builder().uri("/api/x");
        if let Some(value) = header {
            builder = builder.header(CLIENT_IP_HEADER, value);
        }
        let mut request = builder.body(Body::empty()).unwrap();
        if let Some(addr) = peer {
            request
                .extensions_mut()
                .insert(ConnectInfo(addr.parse::<SocketAddr>().unwrap()));
        }
        request
    }

    #[test]
    fn prefers_cloudflare_client_ip() {
        let ip = client_ip(&request(Some("203.0.113.7"), Some("10.0.0.5:1234")));
        assert_eq!(ip, "203.0.113.7".parse::<IpAddr>().unwrap());
    }

    #[test]
    fn buckets_ipv6_clients_by_their_64_prefix() {
        let a = client_ip(&request(Some("2001:db8:1:2:aaaa::1"), None));
        let b = client_ip(&request(Some("2001:db8:1:2:ffff:ffff:ffff:ffff"), None));
        let other = client_ip(&request(Some("2001:db8:1:3::1"), None));

        assert_eq!(a, "2001:db8:1:2::".parse::<IpAddr>().unwrap());
        assert_eq!(a, b);
        assert_ne!(a, other);
    }

    #[test]
    fn unwraps_ipv4_mapped_addresses() {
        let ip = client_ip(&request(Some("::ffff:203.0.113.9"), None));
        assert_eq!(ip, "203.0.113.9".parse::<IpAddr>().unwrap());
    }

    #[test]
    fn falls_back_to_peer_address_when_header_is_missing_or_garbage() {
        let expected = "10.0.0.5".parse::<IpAddr>().unwrap();
        assert_eq!(client_ip(&request(None, Some("10.0.0.5:1234"))), expected);
        assert_eq!(
            client_ip(&request(Some("not-an-ip"), Some("10.0.0.5:1234"))),
            expected
        );
    }

    #[test]
    fn unknown_clients_share_one_bucket() {
        assert_eq!(
            client_ip(&request(None, None)),
            IpAddr::V4(Ipv4Addr::UNSPECIFIED)
        );
    }

    #[test]
    fn auth_limiter_blocks_after_quota() {
        let limits = RateLimits::new();
        let ip = IpAddr::V4(Ipv4Addr::LOCALHOST);
        for _ in 0..AUTH_PER_MINUTE.get() {
            assert!(limits.auth.check_key(&ip).is_ok());
        }
        assert!(limits.auth.check_key(&ip).is_err());
        // Other clients are unaffected.
        assert!(
            limits
                .auth
                .check_key(&IpAddr::V4(Ipv4Addr::BROADCAST))
                .is_ok()
        );
    }
}
