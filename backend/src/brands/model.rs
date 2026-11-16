//! Brand DTOs exposed to the frontend.

use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

use super::wikidata::BrandCandidate;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, sqlx::FromRow, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BrandDto {
    pub id: Uuid,
    pub wikidata_id: String,
    pub name: String,
    /// Shops found on OpenStreetMap (0 until the first fetch finished).
    #[ts(type = "number")]
    pub location_count: i64,
    #[ts(type = "number | null")]
    pub last_synced_at: Option<i64>,
}

/// Answer to `GET /api/brands/search?q=`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BrandSearchDto {
    /// Brands already linked to some card on this server.
    pub linked: Vec<BrandDto>,
    /// Wikidata items not linked yet.
    pub candidates: Vec<BrandCandidate>,
    /// Wikidata could not be reached; only `linked` is filled.
    pub remote_unavailable: bool,
}

/// Body of `POST /api/brands`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct LinkBrandInput {
    pub wikidata_id: String,
}

/// Locations of one brand, as `[lat, lon]` pairs to keep the payload small.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BrandLocationsDto {
    pub brand_id: Uuid,
    pub points: Vec<(f64, f64)>,
}

/// Answer to `GET /api/locations?version=`.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LocationsDto {
    /// Pass back as `version`; changes when brands or their shops change.
    pub version: String,
    /// "Nearby" distance configured on the server, in metres.
    pub radius_m: u32,
    /// Shops of every brand linked to a card the caller can see, or `null` when unchanged.
    pub brands: Option<Vec<BrandLocationsDto>>,
}
