//! `/api/cards/*` and `/api/images/*` endpoints.

use axum::body::Bytes;
use axum::extract::rejection::{BytesRejection, QueryRejection};
use axum::extract::{DefaultBodyLimit, Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post, put};
use axum::{Json, Router};
use uuid::Uuid;

use super::images::{self, MAX_IMAGE_BYTES};
use super::model::{CardDto, CardInput, CardPrefsInput, ChangesDto, ImageCreated, ImageKind};
use super::policy::Viewer;
use super::service;
use crate::accounts::sessions::CurrentUser;
use crate::clock::now_unix;
use crate::error::{ApiError, ApiJson, ApiPath};
use crate::state::AppState;

/// Image ids change whenever a photo is replaced, so a response never goes stale.
const IMAGE_CACHE_CONTROL: &str = "private, max-age=31536000, immutable";

pub fn routes() -> Router<AppState> {
    let photos = Router::new()
        .route(
            "/cards/{id}/images/{kind}",
            put(put_image).delete(delete_image),
        )
        .layer(DefaultBodyLimit::max(MAX_IMAGE_BYTES));

    Router::new()
        .route("/cards", get(list_cards))
        .route("/changes", get(changes))
        .route("/cards/{id}", put(put_card).delete(delete_card))
        .route("/cards/{id}/prefs", put(put_prefs))
        .route("/cards/{id}/use", post(record_use))
        .route("/images/{id}", get(get_image))
        .merge(photos)
}

async fn viewer(state: &AppState, user: CurrentUser) -> Result<Viewer, ApiError> {
    service::viewer_of(&state.db, &user.0).await
}

async fn list_cards(
    State(state): State<AppState>,
    user: CurrentUser,
) -> Result<Json<Vec<CardDto>>, ApiError> {
    let viewer = viewer(&state, user).await?;
    service::list_cards(&state.db, viewer).await.map(Json)
}

#[derive(Debug, serde::Deserialize)]
struct ChangesQuery {
    #[serde(default)]
    since: i64,
}

async fn changes(
    State(state): State<AppState>,
    user: CurrentUser,
    query: Result<Query<ChangesQuery>, QueryRejection>,
) -> Result<Json<ChangesDto>, ApiError> {
    let Query(query) = query.map_err(|e| ApiError::Validation(e.body_text()))?;
    let viewer = viewer(&state, user).await?;
    service::changes_since(&state.db, viewer, query.since)
        .await
        .map(Json)
}

async fn put_card(
    State(state): State<AppState>,
    user: CurrentUser,
    ApiPath(id): ApiPath<Uuid>,
    ApiJson(input): ApiJson<CardInput>,
) -> Result<Json<CardDto>, ApiError> {
    let viewer = viewer(&state, user).await?;
    service::put_card(&state.db, viewer, id, input, now_unix())
        .await
        .map(Json)
}

async fn delete_card(
    State(state): State<AppState>,
    user: CurrentUser,
    ApiPath(id): ApiPath<Uuid>,
) -> Result<StatusCode, ApiError> {
    let viewer = viewer(&state, user).await?;
    service::delete_card(&state.db, viewer, id, now_unix()).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn put_prefs(
    State(state): State<AppState>,
    user: CurrentUser,
    ApiPath(id): ApiPath<Uuid>,
    ApiJson(input): ApiJson<CardPrefsInput>,
) -> Result<Json<CardDto>, ApiError> {
    let viewer = viewer(&state, user).await?;
    service::set_favorite(&state.db, viewer, id, input.is_favorite)
        .await
        .map(Json)
}

async fn record_use(
    State(state): State<AppState>,
    user: CurrentUser,
    ApiPath(id): ApiPath<Uuid>,
) -> Result<StatusCode, ApiError> {
    let viewer = viewer(&state, user).await?;
    service::record_use(&state.db, viewer, id, now_unix()).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn put_image(
    State(state): State<AppState>,
    user: CurrentUser,
    ApiPath((card_id, kind)): ApiPath<(Uuid, ImageKind)>,
    headers: HeaderMap,
    body: Result<Bytes, BytesRejection>,
) -> Result<Json<ImageCreated>, ApiError> {
    let content_type = headers
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok());
    let body = body?;
    let mime = images::check_upload(content_type, &body)?;
    let viewer = viewer(&state, user).await?;
    let id = service::put_image(&state.db, viewer, card_id, kind, mime, &body, now_unix()).await?;
    Ok(Json(ImageCreated { id }))
}

async fn delete_image(
    State(state): State<AppState>,
    user: CurrentUser,
    ApiPath((card_id, kind)): ApiPath<(Uuid, ImageKind)>,
) -> Result<StatusCode, ApiError> {
    let viewer = viewer(&state, user).await?;
    service::delete_image(&state.db, viewer, card_id, kind, now_unix()).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn get_image(
    State(state): State<AppState>,
    user: CurrentUser,
    ApiPath(id): ApiPath<Uuid>,
) -> Result<Response, ApiError> {
    let viewer = viewer(&state, user).await?;
    let image = service::get_image(&state.db, viewer, id).await?;
    let content_type = HeaderValue::from_str(&image.mime)
        .map_err(|e| ApiError::internal("stored image type", e))?;
    Ok((
        [
            (header::CONTENT_TYPE, content_type),
            (
                header::CACHE_CONTROL,
                HeaderValue::from_static(IMAGE_CACHE_CONTROL),
            ),
        ],
        image.data,
    )
        .into_response())
}
