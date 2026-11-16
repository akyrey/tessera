//! `/api/brands/*` and `/api/locations` endpoints.

use axum::extract::rejection::QueryRejection;
use axum::extract::{Query, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router, middleware};
use serde::Deserialize;
use uuid::Uuid;

use super::model::{BrandDto, BrandSearchDto, LinkBrandInput, LocationsDto};
use super::service;
use crate::accounts::sessions::CurrentUser;
use crate::clock::now_unix;
use crate::error::{ApiError, ApiJson, ApiPath};
use crate::http::rate_limit;
use crate::state::AppState;

pub fn routes(state: &AppState) -> Router<AppState> {
    // Both call Wikidata on behalf of the user: keep them well below its limits.
    let upstream = Router::new()
        .route("/brands/search", get(search))
        .route("/brands", post(link))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            rate_limit::limit_upstream,
        ));
    Router::new()
        .route("/locations", get(locations))
        .route("/brands/logos/{id}", get(logo))
        .merge(upstream)
}

/// Logo ids change whenever a logo is replaced, so a response never goes stale.
const LOGO_CACHE_CONTROL: &str = "private, max-age=31536000, immutable";

async fn logo(
    State(state): State<AppState>,
    _user: CurrentUser,
    ApiPath(id): ApiPath<Uuid>,
) -> Result<Response, ApiError> {
    let (mime, data) = service::get_logo(&state.db, id).await?;
    let content_type =
        HeaderValue::from_str(&mime).map_err(|e| ApiError::internal("stored logo type", e))?;
    Ok((
        [
            (header::CONTENT_TYPE, content_type),
            (
                header::CACHE_CONTROL,
                HeaderValue::from_static(LOGO_CACHE_CONTROL),
            ),
        ],
        data,
    )
        .into_response())
}

#[derive(Debug, Deserialize)]
struct SearchQuery {
    q: String,
}

async fn search(
    State(state): State<AppState>,
    user: CurrentUser,
    query: Result<Query<SearchQuery>, QueryRejection>,
) -> Result<Json<BrandSearchDto>, ApiError> {
    let Query(query) = query.map_err(|e| ApiError::Validation(e.body_text()))?;
    service::search(&state.db, &state.upstream, user.0.id, &query.q)
        .await
        .map(Json)
}

async fn link(
    State(state): State<AppState>,
    _user: CurrentUser,
    ApiJson(input): ApiJson<LinkBrandInput>,
) -> Result<(StatusCode, Json<BrandDto>), ApiError> {
    let (brand, created) = service::link(
        &state.db,
        &state.upstream,
        input.wikidata_id.trim(),
        now_unix(),
    )
    .await?;
    // Shops are fetched once a card actually uses the brand (see the card handlers).
    let status = if created {
        StatusCode::CREATED
    } else {
        StatusCode::OK
    };
    Ok((status, Json(brand)))
}

#[derive(Debug, Deserialize)]
struct LocationsQuery {
    version: Option<String>,
}

async fn locations(
    State(state): State<AppState>,
    user: CurrentUser,
    query: Result<Query<LocationsQuery>, QueryRejection>,
) -> Result<Json<LocationsDto>, ApiError> {
    let Query(query) = query.map_err(|e| ApiError::Validation(e.body_text()))?;
    service::locations_for(
        &state.db,
        user.0.id,
        query.version.as_deref(),
        state.config.nearby_radius_m,
    )
    .await
    .map(Json)
}
