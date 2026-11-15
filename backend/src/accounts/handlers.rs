//! `/api/auth/*`, `/api/me/*` and `/api/household/*` endpoints.

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post};
use axum::{Json, Router, middleware};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;
use webauthn_rs::prelude::{
    Passkey, PasskeyRegistration, PublicKeyCredential, RegisterPublicKeyCredential,
};

use super::model::{InviteCreated, InvitePreview, Me, PasskeyDto, User};
use super::service::{self, NewCredential, PendingRegistration};
use super::sessions::{self, CurrentUser};
use super::{passkeys, repo};
use crate::clock::now_unix;
use crate::error::{ApiError, ApiJson};
use crate::http::rate_limit::{self, ClientIp};
use crate::state::AppState;
use crate::tokens;

// ---------- request / response bodies ----------

#[derive(Debug, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct InviteTokenRequest {
    pub token: String,
}

#[derive(Debug, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct RegisterStartRequest {
    pub token: String,
    #[ts(optional)]
    pub display_name: Option<String>,
}

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CeremonyStart {
    pub ceremony_id: Uuid,
    /// WebAuthn options (`{ publicKey: … }`) to pass to the browser.
    #[ts(type = "unknown")]
    pub options: serde_json::Value,
}

#[derive(Debug, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct RegisterFinishRequest {
    pub ceremony_id: Uuid,
    #[ts(type = "unknown")]
    pub credential: RegisterPublicKeyCredential,
    #[ts(optional)]
    pub passkey_name: Option<String>,
}

#[derive(Debug, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct LoginFinishRequest {
    pub ceremony_id: Uuid,
    #[ts(type = "unknown")]
    pub credential: PublicKeyCredential,
}

/// Server-side state between `…/start` and `…/finish`.
pub struct RegistrationCeremony {
    purpose: RegistrationPurpose,
    state: PasskeyRegistration,
}

enum RegistrationPurpose {
    Invite(PendingRegistration),
    AddPasskey { user_id: Uuid },
}

pub fn routes(state: &AppState) -> Router<AppState> {
    let auth = Router::new()
        .route("/invite", post(preview_invite))
        .route("/register/start", post(register_start))
        .route("/register/finish", post(register_finish))
        .route("/login/start", post(login_start))
        .route("/login/finish", post(login_finish))
        .route("/logout", post(logout))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            rate_limit::limit_auth,
        ));

    Router::new()
        .nest("/auth", auth)
        .route("/me", get(me))
        .route("/me/passkeys", get(list_passkeys))
        .route("/me/passkeys/start", post(add_passkey_start))
        .route("/me/passkeys/finish", post(add_passkey_finish))
        .route("/me/passkeys/{id}", delete(remove_passkey))
        .route("/me/sessions", delete(sign_out_everywhere))
        .route("/household/invites", post(create_household_invite))
}

// ---------- invites & registration ----------

async fn preview_invite(
    State(state): State<AppState>,
    ApiJson(body): ApiJson<InviteTokenRequest>,
) -> Result<Json<InvitePreview>, ApiError> {
    service::preview_invite(&state.db, &body.token, now_unix())
        .await
        .map(Json)
}

async fn register_start(
    State(state): State<AppState>,
    client: ClientIp,
    ApiJson(body): ApiJson<RegisterStartRequest>,
) -> Result<Json<CeremonyStart>, ApiError> {
    let pending = service::plan_registration(
        &state.db,
        &body.token,
        body.display_name.as_deref(),
        now_unix(),
    )
    .await?;
    let exclude = repo::credential_ids_of(&state.db, pending.user_id).await?;
    start_registration_ceremony(
        &state,
        client,
        pending.user_id,
        &pending.display_name.clone(),
        exclude,
        |s| RegistrationCeremony {
            purpose: RegistrationPurpose::Invite(pending),
            state: s,
        },
    )
}

async fn register_finish(
    State(state): State<AppState>,
    ApiJson(body): ApiJson<RegisterFinishRequest>,
) -> Result<Response, ApiError> {
    let ceremony = state
        .registrations
        .take(body.ceremony_id)
        .ok_or(ApiError::CeremonyExpired)?;
    let RegistrationPurpose::Invite(pending) = ceremony.purpose else {
        return Err(ApiError::CeremonyExpired);
    };
    let passkey = verify_registration(&state, &body.credential, &ceremony.state)?;
    let name = passkey_name(body.passkey_name.as_deref())?;
    let passkey_json = to_json(&passkey)?;
    let credential = NewCredential {
        credential_id: passkey.cred_id().as_ref(),
        passkey_json: &passkey_json,
        name: &name,
    };
    let user = service::complete_registration(&state.db, &pending, &credential, now_unix()).await?;
    signed_in(&state, &user).await
}

// ---------- login / logout ----------

async fn login_start(
    State(state): State<AppState>,
    ClientIp(client): ClientIp,
) -> Result<Json<CeremonyStart>, ApiError> {
    let (options, ceremony) =
        passkeys::start_login(&state.webauthn).map_err(|e| ApiError::internal("start login", e))?;
    let ceremony_id = state
        .logins
        .insert(client, ceremony)
        .map_err(|_| ApiError::RateLimited)?;
    Ok(Json(CeremonyStart {
        ceremony_id,
        options: to_value(&options)?,
    }))
}

async fn login_finish(
    State(state): State<AppState>,
    ApiJson(body): ApiJson<LoginFinishRequest>,
) -> Result<Response, ApiError> {
    let ceremony = state
        .logins
        .take(body.ceremony_id)
        .ok_or(ApiError::CeremonyExpired)?;
    let (user_id, credential_id) = state
        .webauthn
        .identify_discoverable_authentication(&body.credential)
        .map_err(|_| ApiError::AuthenticationFailed)?;
    let stored = repo::find_passkey_by_credential(&state.db, credential_id)
        .await?
        .filter(|passkey| passkey.user_id == user_id)
        .ok_or(ApiError::AuthenticationFailed)?;

    let mut passkey: Passkey = serde_json::from_str(&stored.passkey)
        .map_err(|e| ApiError::internal("stored passkey", e))?;
    let result = state
        .webauthn
        .finish_discoverable_authentication_v2(&body.credential, ceremony, &passkey)
        .map_err(|error| {
            tracing::warn!(%error, %user_id, "passkey authentication rejected");
            ApiError::AuthenticationFailed
        })?;

    let changed = passkey.update_credential(&result) == Some(true);
    let updated_json = changed.then(|| to_json(&passkey)).transpose()?;
    repo::touch_passkey(&state.db, stored.id, updated_json.as_deref(), now_unix()).await?;

    let user = repo::find_user(&state.db, user_id)
        .await?
        .ok_or(ApiError::AuthenticationFailed)?;
    signed_in(&state, &user).await
}

async fn logout(State(state): State<AppState>, headers: HeaderMap) -> Result<Response, ApiError> {
    if let Some(token) = state.cookie.read(&headers) {
        sessions::end_session(&state.db, token).await?;
    }
    Ok((
        StatusCode::NO_CONTENT,
        [(header::SET_COOKIE, state.cookie.clear())],
    )
        .into_response())
}

// ---------- profile & passkeys ----------

async fn me(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
) -> Result<Json<Me>, ApiError> {
    service::load_me(&state.db, &user).await.map(Json)
}

async fn list_passkeys(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
) -> Result<Json<Vec<PasskeyDto>>, ApiError> {
    let passkeys = repo::passkeys_of(&state.db, user.id).await?;
    Ok(Json(passkeys.into_iter().map(PasskeyDto::from).collect()))
}

async fn add_passkey_start(
    State(state): State<AppState>,
    client: ClientIp,
    CurrentUser(user): CurrentUser,
) -> Result<Json<CeremonyStart>, ApiError> {
    let exclude = repo::credential_ids_of(&state.db, user.id).await?;
    start_registration_ceremony(&state, client, user.id, &user.display_name, exclude, |s| {
        RegistrationCeremony {
            purpose: RegistrationPurpose::AddPasskey { user_id: user.id },
            state: s,
        }
    })
}

async fn add_passkey_finish(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    ApiJson(body): ApiJson<RegisterFinishRequest>,
) -> Result<Json<PasskeyDto>, ApiError> {
    let ceremony = state
        .registrations
        .take(body.ceremony_id)
        .ok_or(ApiError::CeremonyExpired)?;
    match ceremony.purpose {
        RegistrationPurpose::AddPasskey { user_id } if user_id == user.id => {}
        _ => return Err(ApiError::CeremonyExpired),
    }
    let passkey = verify_registration(&state, &body.credential, &ceremony.state)?;
    let name = passkey_name(body.passkey_name.as_deref())?;
    let passkey_json = to_json(&passkey)?;
    let now = now_unix();
    let credential = NewCredential {
        credential_id: passkey.cred_id().as_ref(),
        passkey_json: &passkey_json,
        name: &name,
    };
    let id = service::add_passkey(&state.db, user.id, &credential, now).await?;
    Ok(Json(PasskeyDto {
        id,
        name,
        created_at: now,
        last_used_at: None,
    }))
}

async fn remove_passkey(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    let current = state.cookie.read(&headers).map(tokens::hash);
    service::delete_passkey(&state.db, user.id, id, current.as_deref()).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn sign_out_everywhere(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
) -> Result<Response, ApiError> {
    sessions::end_all_sessions(&state.db, user.id).await?;
    Ok((
        StatusCode::NO_CONTENT,
        [(header::SET_COOKIE, state.cookie.clear())],
    )
        .into_response())
}

async fn create_household_invite(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
) -> Result<Json<InviteCreated>, ApiError> {
    let invite = service::create_household_invite(&state.db, &user, now_unix()).await?;
    Ok(Json(InviteCreated {
        url: invite_url(&state, &invite.token.value),
        expires_at: invite.expires_at,
    }))
}

// ---------- helpers ----------

/// Link shared with the invitee; the secret lives in the fragment, which
/// browsers never send to the server (or to Cloudflare's logs).
pub fn invite_url(state: &AppState, token: &str) -> String {
    format!("{}/join#{token}", state.config.public_origin())
}

fn start_registration_ceremony(
    state: &AppState,
    ClientIp(client): ClientIp,
    user_id: Uuid,
    display_name: &str,
    exclude: Vec<Vec<u8>>,
    ceremony: impl FnOnce(PasskeyRegistration) -> RegistrationCeremony,
) -> Result<Json<CeremonyStart>, ApiError> {
    let (options, registration) =
        passkeys::start_registration(&state.webauthn, user_id, display_name, exclude)
            .map_err(|e| ApiError::internal("start registration", e))?;
    let ceremony_id = state
        .registrations
        .insert(client, ceremony(registration))
        .map_err(|_| ApiError::RateLimited)?;
    Ok(Json(CeremonyStart {
        ceremony_id,
        options: to_value(&options)?,
    }))
}

fn verify_registration(
    state: &AppState,
    credential: &RegisterPublicKeyCredential,
    registration: &PasskeyRegistration,
) -> Result<Passkey, ApiError> {
    state
        .webauthn
        .finish_passkey_registration(credential, registration)
        .map_err(|error| {
            tracing::warn!(%error, "passkey registration rejected");
            ApiError::Validation("the passkey could not be verified".into())
        })
}

fn passkey_name(raw: Option<&str>) -> Result<String, ApiError> {
    match raw.map(str::trim).filter(|name| !name.is_empty()) {
        Some(name) => service::validate_name("passkey name", name),
        None => Ok(service::DEFAULT_PASSKEY_NAME.to_owned()),
    }
}

async fn signed_in(state: &AppState, user: &User) -> Result<Response, ApiError> {
    let token =
        sessions::start_session(&state.db, user.id, state.cookie.ttl_secs(), now_unix()).await?;
    let me = service::load_me(&state.db, user).await?;
    Ok((
        [(header::SET_COOKIE, state.cookie.set(&token.value))],
        Json(me),
    )
        .into_response())
}

fn to_value(value: &impl Serialize) -> Result<serde_json::Value, ApiError> {
    serde_json::to_value(value).map_err(|e| ApiError::internal("serialize options", e))
}

fn to_json(passkey: &Passkey) -> Result<String, ApiError> {
    serde_json::to_string(passkey).map_err(|e| ApiError::internal("serialize passkey", e))
}
