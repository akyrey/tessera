//! `/api/brands/*` and `/api/locations` endpoints.

use axum::extract::rejection::QueryRejection;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router, middleware};
use serde::Deserialize;

use super::model::{BrandDto, BrandSearchDto, LinkBrandInput, LocationsDto};
use super::{refresh, service};
use crate::accounts::sessions::CurrentUser;
use crate::clock::now_unix;
use crate::error::{ApiError, ApiJson};
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
        .merge(upstream)
}

#[derive(Debug, Deserialize)]
struct SearchQuery {
    q: String,
}

async fn search(
    State(state): State<AppState>,
    _user: CurrentUser,
    query: Result<Query<SearchQuery>, QueryRejection>,
) -> Result<Json<BrandSearchDto>, ApiError> {
    let Query(query) = query.map_err(|e| ApiError::Validation(e.body_text()))?;
    service::search(&state.db, &state.upstream, &query.q)
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
    if created {
        refresh::refresh_soon(&state, brand.id, brand.wikidata_id.clone());
    }
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
