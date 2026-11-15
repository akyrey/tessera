//! HTTP tests for sign-in, sessions, passkeys and household invites.
//!
//! The WebAuthn ceremony itself (browser + authenticator) is exercised by the
//! Playwright suite with a virtual authenticator; here we test everything
//! around it, including how the endpoints reject bad input.

mod common;

use axum::http::{StatusCode, header};
use common::{COOKIE_NAME, ORIGIN, error_code, json, json_request, spawn_app};
use serde_json::json;
use tessera::accounts::service;
use tessera::clock::now_unix;
use tessera::janitor;
use tessera::tokens::SecretToken;

// ---------- sessions & profile ----------

#[tokio::test]
async fn me_requires_a_session() {
    let app = spawn_app().await;

    let response = app.get("/api/me", None).await;

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(error_code(response).await, "unauthorized");
}

#[tokio::test]
async fn me_rejects_unknown_session_tokens() {
    let app = spawn_app().await;
    let forged = SecretToken::generate().value;

    let response = app.get("/api/me", Some(&forged)).await;

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn me_returns_user_and_household() {
    let app = spawn_app().await;
    let (user, session) = app.bootstrap_admin("Alice").await;

    let response = app.get("/api/me", Some(&session)).await;

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    let body = json(response).await;
    assert_eq!(body["user"]["id"], user.id.to_string());
    assert_eq!(body["user"]["displayName"], "Alice");
    assert_eq!(body["user"]["isAdmin"], true);
    assert_eq!(body["household"]["role"], "owner");
    assert_eq!(body["household"]["members"][0]["displayName"], "Alice");
}

#[tokio::test]
async fn expired_sessions_are_rejected_and_swept() {
    let app = spawn_app().await;
    let (_, session) = app.bootstrap_admin("Alice").await;
    let far_future = now_unix() + 10 * 365 * 24 * 3600;

    let swept = janitor::sweep(&app.state, far_future).await.unwrap();

    assert_eq!(swept.sessions, 1);
    assert_eq!(
        app.get("/api/me", Some(&session)).await.status(),
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn logout_ends_the_session_and_clears_the_cookie() {
    let app = spawn_app().await;
    let (_, session) = app.bootstrap_admin("Alice").await;

    let response = app
        .post("/api/auth/logout", serde_json::Value::Null, Some(&session))
        .await;

    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    let cookie = response.headers()[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .to_owned();
    assert!(cookie.starts_with(&format!("{COOKIE_NAME}=;")), "{cookie}");
    assert!(cookie.contains("Max-Age=0"));
    assert_eq!(
        app.get("/api/me", Some(&session)).await.status(),
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn logout_without_session_still_succeeds() {
    let app = spawn_app().await;
    let response = app
        .post("/api/auth/logout", serde_json::Value::Null, None)
        .await;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
}

// ---------- CSRF / Origin ----------

#[tokio::test]
async fn state_changing_requests_need_our_origin() {
    let app = spawn_app().await;
    let (_, session) = app.bootstrap_admin("Alice").await;

    for origin in [None, Some("https://evil.example")] {
        let request = json_request(
            "POST",
            "/api/household/invites",
            serde_json::Value::Null,
            Some(&session),
            origin,
        );
        let response = app.send(request).await;
        assert_eq!(response.status(), StatusCode::FORBIDDEN, "{origin:?}");
        assert_eq!(error_code(response).await, "bad_origin");
    }
}

// ---------- invites ----------

#[tokio::test]
async fn invite_preview_describes_a_valid_bootstrap_link() {
    let app = spawn_app().await;
    let invite = service::ensure_bootstrap_invite(&app.state.db, None, now_unix())
        .await
        .unwrap()
        .unwrap();

    let response = app
        .post(
            "/api/auth/invite",
            json!({ "token": invite.token.value }),
            None,
        )
        .await;

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(json(response).await["kind"], "bootstrap");
}

#[tokio::test]
async fn invite_preview_rejects_bad_tokens_with_410() {
    let app = spawn_app().await;

    let response = app
        .post("/api/auth/invite", json!({ "token": "nope" }), None)
        .await;

    assert_eq!(response.status(), StatusCode::GONE);
    assert_eq!(error_code(response).await, "invalid_invite");
}

#[tokio::test]
async fn owner_creates_a_shareable_household_link() {
    let app = spawn_app().await;
    let (_, session) = app.bootstrap_admin("Alice").await;

    let response = app
        .post(
            "/api/household/invites",
            serde_json::Value::Null,
            Some(&session),
        )
        .await;

    assert_eq!(response.status(), StatusCode::OK);
    let body = json(response).await;
    let url = body["url"].as_str().unwrap();
    let token = url.strip_prefix(&format!("{ORIGIN}/join#")).expect(url);
    assert!(body["expiresAt"].as_i64().unwrap() > now_unix());

    let preview = json(
        app.post("/api/auth/invite", json!({ "token": token }), None)
            .await,
    )
    .await;
    assert_eq!(preview["kind"], "household");
    assert_eq!(preview["householdName"], "Home");
}

#[tokio::test]
async fn members_cannot_create_invites() {
    let app = spawn_app().await;
    let (owner, _) = app.bootstrap_admin("Alice").await;
    let invite = service::create_household_invite(&app.state.db, &owner, now_unix())
        .await
        .unwrap();
    let (_, member_session) = app.register(&invite.token.value, "Bea").await;

    let response = app
        .post(
            "/api/household/invites",
            serde_json::Value::Null,
            Some(&member_session),
        )
        .await;

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

// ---------- ceremonies ----------

#[tokio::test]
async fn register_start_returns_discoverable_passkey_options() {
    let app = spawn_app().await;
    let invite = service::ensure_bootstrap_invite(&app.state.db, None, now_unix())
        .await
        .unwrap()
        .unwrap();

    let response = app
        .post(
            "/api/auth/register/start",
            json!({ "token": invite.token.value, "displayName": "Alice" }),
            None,
        )
        .await;

    assert_eq!(response.status(), StatusCode::OK);
    let body = json(response).await;
    assert!(body["ceremonyId"].is_string());
    let public_key = &body["options"]["publicKey"];
    assert_eq!(public_key["rp"]["id"], "localhost");
    assert_eq!(public_key["user"]["name"], "Alice");
    assert_eq!(
        public_key["authenticatorSelection"]["residentKey"],
        "required"
    );
}

#[tokio::test]
async fn register_start_validates_display_name() {
    let app = spawn_app().await;
    let invite = service::ensure_bootstrap_invite(&app.state.db, None, now_unix())
        .await
        .unwrap()
        .unwrap();

    let response = app
        .post(
            "/api/auth/register/start",
            json!({ "token": invite.token.value, "displayName": "  " }),
            None,
        )
        .await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(error_code(response).await, "invalid_request");
}

#[tokio::test]
async fn unknown_fields_are_rejected_with_the_error_envelope() {
    let app = spawn_app().await;

    let response = app
        .post(
            "/api/auth/invite",
            json!({ "token": "x", "admin": true }),
            None,
        )
        .await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(error_code(response).await, "invalid_request");
}

#[tokio::test]
async fn finishing_an_unknown_ceremony_is_rejected() {
    let app = spawn_app().await;
    let credential = json!({
        "id": "AAAA", "rawId": "AAAA", "type": "public-key",
        "response": { "clientDataJSON": "AAAA", "authenticatorData": "AAAA", "signature": "AAAA", "userHandle": null },
        "extensions": {}
    });

    let response = app
        .post(
            "/api/auth/login/finish",
            json!({ "ceremonyId": uuid::Uuid::new_v4(), "credential": credential }),
            None,
        )
        .await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(error_code(response).await, "ceremony_expired");
}

#[tokio::test]
async fn forged_login_assertion_fails_and_burns_the_ceremony() {
    let app = spawn_app().await;
    app.bootstrap_admin("Alice").await;
    let start = json(
        app.post("/api/auth/login/start", serde_json::Value::Null, None)
            .await,
    )
    .await;
    assert!(
        start["options"]
            .get("mediation")
            .is_none_or(serde_json::Value::is_null)
    );
    let ceremony_id = start["ceremonyId"].clone();
    let forged = json!({
        "id": "AAAA", "rawId": "AAAA", "type": "public-key",
        "response": { "clientDataJSON": "AAAA", "authenticatorData": "AAAA", "signature": "AAAA", "userHandle": "AAAA" },
        "extensions": {}
    });

    let first = app
        .post(
            "/api/auth/login/finish",
            json!({ "ceremonyId": ceremony_id, "credential": forged }),
            None,
        )
        .await;
    let replay = app
        .post(
            "/api/auth/login/finish",
            json!({ "ceremonyId": ceremony_id, "credential": forged }),
            None,
        )
        .await;

    assert_eq!(first.status(), StatusCode::UNAUTHORIZED);
    assert!(first.headers().get(header::SET_COOKIE).is_none());
    assert_eq!(error_code(replay).await, "ceremony_expired");
}

#[tokio::test]
async fn add_passkey_ceremony_cannot_be_finished_by_another_user() {
    let app = spawn_app().await;
    let (owner, owner_session) = app.bootstrap_admin("Alice").await;
    let invite = service::create_household_invite(&app.state.db, &owner, now_unix())
        .await
        .unwrap();
    let (_, bea_session) = app.register(&invite.token.value, "Bea").await;
    let start = json(
        app.post(
            "/api/me/passkeys/start",
            serde_json::Value::Null,
            Some(&owner_session),
        )
        .await,
    )
    .await;
    let credential = json!({
        "id": "AAAA", "rawId": "AAAA", "type": "public-key",
        "response": { "attestationObject": "AAAA", "clientDataJSON": "AAAA" },
        "extensions": {}
    });

    let response = app
        .post(
            "/api/me/passkeys/finish",
            json!({ "ceremonyId": start["ceremonyId"], "credential": credential }),
            Some(&bea_session),
        )
        .await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(error_code(response).await, "ceremony_expired");
}

// ---------- passkey management ----------

#[tokio::test]
async fn lists_passkeys_and_protects_the_last_one() {
    let app = spawn_app().await;
    let (_, session) = app.bootstrap_admin("Alice").await;

    let passkeys = json(app.get("/api/me/passkeys", Some(&session)).await).await;
    let only = passkeys[0]["id"].as_str().unwrap().to_owned();
    assert_eq!(passkeys.as_array().unwrap().len(), 1);
    assert_eq!(passkeys[0]["name"], "Test device");

    let response = app
        .delete(&format!("/api/me/passkeys/{only}"), Some(&session))
        .await;
    assert_eq!(response.status(), StatusCode::CONFLICT);

    let response = app
        .delete(
            &format!("/api/me/passkeys/{}", uuid::Uuid::new_v4()),
            Some(&session),
        )
        .await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

// ---------- rate limiting ----------

fn from_ip(
    method: &str,
    uri: &str,
    body: serde_json::Value,
    ip: &'static str,
) -> axum::http::Request<axum::body::Body> {
    let mut request = json_request(method, uri, body, None, Some(ORIGIN));
    request
        .headers_mut()
        .insert("cf-connecting-ip", ip.parse().unwrap());
    request
}

#[tokio::test]
async fn auth_endpoints_are_rate_limited_per_client() {
    let app = spawn_app().await;
    let probe = |ip| {
        app.send(from_ip(
            "POST",
            "/api/auth/invite",
            json!({ "token": "nope" }),
            ip,
        ))
    };

    for _ in 0..20 {
        assert_eq!(probe("203.0.113.1").await.status(), StatusCode::GONE);
    }
    let blocked = probe("203.0.113.1").await;
    assert_eq!(blocked.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(error_code(blocked).await, "rate_limited");
    assert_eq!(probe("203.0.113.2").await.status(), StatusCode::GONE);
}

#[tokio::test]
async fn ipv6_clients_share_a_bucket_per_64() {
    let app = spawn_app().await;
    let ips = [
        "2001:db8:1:2::1",
        "2001:db8:1:2::2",
        "2001:db8:1:2:ffff::3",
        "2001:db8:1:2::4",
    ];

    for n in 0..20 {
        let response = app
            .send(from_ip(
                "POST",
                "/api/auth/invite",
                json!({ "token": "x" }),
                ips[n % ips.len()],
            ))
            .await;
        assert_eq!(response.status(), StatusCode::GONE);
    }
    let rotated = app
        .send(from_ip(
            "POST",
            "/api/auth/invite",
            json!({ "token": "x" }),
            "2001:db8:1:2::99",
        ))
        .await;
    assert_eq!(rotated.status(), StatusCode::TOO_MANY_REQUESTS);
}

#[tokio::test]
async fn one_client_cannot_exhaust_sign_in_ceremonies() {
    let app = spawn_app().await;
    let start = |ip| {
        app.send(from_ip(
            "POST",
            "/api/auth/login/start",
            serde_json::Value::Null,
            ip,
        ))
    };

    for _ in 0..8 {
        assert_eq!(start("198.51.100.7").await.status(), StatusCode::OK);
    }
    assert_eq!(
        start("198.51.100.7").await.status(),
        StatusCode::TOO_MANY_REQUESTS
    );
    // Everyone else can still sign in.
    assert_eq!(start("203.0.113.1").await.status(), StatusCode::OK);
}

// ---------- session revocation ----------

#[tokio::test]
async fn sign_out_everywhere_ends_all_sessions() {
    let app = spawn_app().await;
    let (user, phone) = app.bootstrap_admin("Alice").await;
    let laptop =
        tessera::accounts::sessions::start_session(&app.state.db, user.id, 3600, now_unix())
            .await
            .unwrap()
            .value;

    let response = app.delete("/api/me/sessions", Some(&phone)).await;

    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert!(
        response.headers()[header::SET_COOKIE]
            .to_str()
            .unwrap()
            .contains("Max-Age=0")
    );
    assert_eq!(
        app.get("/api/me", Some(&phone)).await.status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        app.get("/api/me", Some(&laptop)).await.status(),
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn removing_a_passkey_keeps_the_current_session_only() {
    let app = spawn_app().await;
    let (user, current) = app.bootstrap_admin("Alice").await;
    let lost_phone =
        tessera::accounts::sessions::start_session(&app.state.db, user.id, 3600, now_unix())
            .await
            .unwrap()
            .value;
    let credential = service::NewCredential {
        credential_id: b"second",
        passkey_json: "{}",
        name: "Mac",
    };
    service::add_passkey(&app.state.db, user.id, &credential, now_unix())
        .await
        .unwrap();
    let passkeys = json(app.get("/api/me/passkeys", Some(&current)).await).await;
    let first = passkeys[0]["id"].as_str().unwrap().to_owned();

    let response = app
        .delete(&format!("/api/me/passkeys/{first}"), Some(&current))
        .await;

    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert_eq!(
        app.get("/api/me", Some(&current)).await.status(),
        StatusCode::OK
    );
    assert_eq!(
        app.get("/api/me", Some(&lost_phone)).await.status(),
        StatusCode::UNAUTHORIZED
    );
}
