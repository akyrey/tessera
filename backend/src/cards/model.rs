//! Card domain types and the DTOs exposed to the frontend.

use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

use super::barcode::BarcodeFormat;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type, TS)]
#[sqlx(rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum ImageKind {
    Front,
    Back,
}

/// Ownership and lifecycle of a card: everything the policy needs.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct CardAccess {
    pub id: Uuid,
    pub owner_id: Uuid,
    pub household_id: Option<Uuid>,
    pub deleted_at: Option<i64>,
}

/// A shop location marked by hand (missing from OpenStreetMap).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct Place {
    pub label: String,
    pub lat: f64,
    pub lon: f64,
}

/// Validated card fields, ready to be stored.
#[derive(Debug, Clone, PartialEq)]
pub struct CardFields {
    pub store_name: String,
    pub card_number: String,
    pub barcode_format: BarcodeFormat,
    pub member_name: Option<String>,
    pub color: Option<String>,
    pub notes: Option<String>,
    pub brand_id: Option<Uuid>,
    /// `None` keeps the stored places (they have their own endpoint).
    pub places: Option<Vec<Place>>,
    pub expires_on: Option<String>,
}

// ---------- DTOs ----------

/// Body of `PUT /api/cards/{id}`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct CardInput {
    pub store_name: String,
    pub card_number: String,
    pub barcode_format: BarcodeFormat,
    #[ts(optional)]
    pub member_name: Option<String>,
    /// `#rrggbb`
    #[ts(optional)]
    pub color: Option<String>,
    #[ts(optional)]
    pub notes: Option<String>,
    /// Store brand, for finding nearby shops.
    #[ts(optional)]
    pub brand_id: Option<Uuid>,
    /// Shops marked by hand; replaces the stored list when present, keeps it when absent.
    #[ts(optional)]
    pub places: Option<Vec<Place>>,
    /// Last valid day (`YYYY-MM-DD`), for coupons and vouchers.
    #[ts(optional)]
    pub expires_on: Option<String>,
}

/// Body of `PUT /api/cards/{id}/places`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct CardPlacesInput {
    pub places: Vec<Place>,
}

/// Body of `PUT /api/cards/{id}/sharing`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct CardSharingInput {
    pub shared: bool,
}

/// Body of `PUT /api/cards/{id}/prefs`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct CardPrefsInput {
    pub is_favorite: bool,
}

/// A card as the signed-in user sees it, including their own preferences.
#[derive(Debug, Clone, PartialEq, Serialize, sqlx::FromRow, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CardDto {
    pub id: Uuid,
    pub owner_id: Uuid,
    /// Display name of the owner, for "added by" on shared cards.
    pub owner_name: String,
    /// Visible to the owner's whole household.
    pub shared: bool,
    pub store_name: String,
    pub card_number: String,
    pub barcode_format: BarcodeFormat,
    pub member_name: Option<String>,
    pub color: Option<String>,
    pub notes: Option<String>,
    pub is_favorite: bool,
    #[ts(type = "number")]
    pub use_count: i64,
    #[ts(type = "number | null")]
    pub last_used_at: Option<i64>,
    pub front_image_id: Option<Uuid>,
    pub back_image_id: Option<Uuid>,
    pub brand_id: Option<Uuid>,
    pub brand_name: Option<String>,
    /// Brand logo, served at `/api/brands/logos/{id}`; a new id whenever it changes.
    pub brand_logo_id: Option<Uuid>,
    pub expires_on: Option<String>,
    /// Filled after the row is read (separate table).
    #[sqlx(skip)]
    pub places: Vec<Place>,
    #[ts(type = "number")]
    pub created_at: i64,
    #[ts(type = "number")]
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ImageCreated {
    pub id: Uuid,
}

/// A stored photo, ready to be served.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct StoredImage {
    pub mime: String,
    pub data: Vec<u8>,
    pub owner_id: Uuid,
    pub household_id: Option<Uuid>,
}

/// Answer to `GET /api/changes?since=`.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ChangesDto {
    /// Pass back as `since` on the next pull.
    #[ts(type = "number")]
    pub cursor: i64,
    /// Every card visible to the caller, or `null` when nothing changed since `since`.
    pub cards: Option<Vec<CardDto>>,
}
