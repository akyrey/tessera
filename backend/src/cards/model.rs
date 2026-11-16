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

/// Validated card fields, ready to be stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CardFields {
    pub store_name: String,
    pub card_number: String,
    pub barcode_format: BarcodeFormat,
    pub member_name: Option<String>,
    pub color: Option<String>,
    pub notes: Option<String>,
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
}

/// Body of `PUT /api/cards/{id}/prefs`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct CardPrefsInput {
    pub is_favorite: bool,
}

/// A card as the signed-in user sees it, including their own preferences.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, sqlx::FromRow, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CardDto {
    pub id: Uuid,
    pub owner_id: Uuid,
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ChangesDto {
    /// Pass back as `since` on the next pull.
    #[ts(type = "number")]
    pub cursor: i64,
    /// Every card visible to the caller, or `null` when nothing changed since `since`.
    pub cards: Option<Vec<CardDto>>,
}
