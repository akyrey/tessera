//! Card use cases: validation, access checks and transactions.
//!
//! Cards a viewer may not see are reported as "not found", so ids of other
//! users' cards cannot be probed.

use sqlx::{SqliteConnection, SqlitePool};
use uuid::Uuid;

use super::barcode::BarcodeFormat;
use super::images::ImageMime;
use super::model::{
    CardAccess, CardDto, CardFields, CardInput, ChangesDto, ImageKind, StoredImage,
};
use super::policy::{self, Ownership, Viewer};
use super::repo;
use crate::accounts::model::User;
use crate::accounts::repo as accounts_repo;
use crate::error::ApiError;

const MAX_STORE_NAME_CHARS: usize = 64;
const MAX_MEMBER_NAME_CHARS: usize = 64;
const MAX_NOTES_CHARS: usize = 1000;
/// Far beyond a real wallet; stops a stolen session from filling the disk.
pub const MAX_CARDS_PER_USER: i64 = 500;
/// Photo storage per card owner (photos are ~100 KB after re-encoding).
pub const MAX_IMAGE_BYTES_PER_USER: i64 = 100 * 1024 * 1024;

pub async fn viewer_of(pool: &SqlitePool, user: &User) -> Result<Viewer, ApiError> {
    let membership = accounts_repo::membership_of(pool, user.id).await?;
    Ok(Viewer {
        user_id: user.id,
        household_id: membership.map(|m| m.household_id),
    })
}

pub async fn list_cards(pool: &SqlitePool, viewer: Viewer) -> Result<Vec<CardDto>, ApiError> {
    Ok(repo::list_visible(pool, viewer.user_id, viewer.household_id).await?)
}

/// Pull sync. With a handful of users and cards a full snapshot is cheaper and
/// simpler than deltas, and it naturally removes deleted or unshared cards.
pub async fn changes_since(
    pool: &SqlitePool,
    viewer: Viewer,
    since: i64,
) -> Result<ChangesDto, ApiError> {
    // One read transaction: the cursor and the snapshot come from the same state.
    let mut tx = pool.begin().await?;
    let cursor = repo::current_change_seq(&mut *tx).await?;
    let cards = if since == cursor {
        None
    } else {
        Some(repo::list_visible(&mut *tx, viewer.user_id, viewer.household_id).await?)
    };
    tx.commit().await?;
    Ok(ChangesDto { cursor, cards })
}

/// Idempotent create-or-replace: the client picks the id (UUIDv7), so a retried
/// request after a lost response does not create a duplicate.
pub async fn put_card(
    pool: &SqlitePool,
    viewer: Viewer,
    id: Uuid,
    input: CardInput,
    now: i64,
) -> Result<CardDto, ApiError> {
    if id.is_nil() || id.is_max() {
        return Err(ApiError::Validation("invalid card id".into()));
    }
    let fields = validate(input)?;

    let mut tx = pool.begin().await?;
    let seq = repo::next_change_seq(&mut *tx).await?;
    match repo::find_access(&mut *tx, id).await? {
        None => {
            if repo::count_live_cards_of(&mut *tx, viewer.user_id).await? >= MAX_CARDS_PER_USER {
                return Err(ApiError::QuotaExceeded(
                    "you have reached the maximum number of cards",
                ));
            }
            repo::insert_card(&mut *tx, id, viewer.user_id, &fields, now, seq).await?;
        }
        Some(card) if policy::can_edit(viewer, ownership(&card)) => {
            repo::update_card(&mut *tx, id, &fields, now, seq).await?;
        }
        // The owner replaying a create or edit after deleting the card elsewhere.
        Some(card) if card.deleted_at.is_some() && card.owner_id == viewer.user_id => {
            return Err(ApiError::CardDeleted);
        }
        Some(_) => return Err(ApiError::NotFound),
    }
    let card = load_dto(&mut tx, viewer, id).await?;
    tx.commit().await?;
    Ok(card)
}

/// Soft delete. Deleting an already deleted card succeeds (idempotent).
pub async fn delete_card(
    pool: &SqlitePool,
    viewer: Viewer,
    id: Uuid,
    now: i64,
) -> Result<(), ApiError> {
    let mut tx = pool.begin().await?;
    let seq = repo::next_change_seq(&mut *tx).await?;
    let card = repo::find_access(&mut *tx, id)
        .await?
        .ok_or(ApiError::NotFound)?;
    if card.deleted_at.is_some() && card.owner_id == viewer.user_id {
        return Ok(());
    }
    let access = ownership(&card);
    if !policy::can_delete(viewer, access) {
        return Err(if policy::can_view(viewer, access) {
            ApiError::Forbidden
        } else {
            ApiError::NotFound
        });
    }
    repo::delete_images_of_card(&mut *tx, id).await?;
    repo::delete_prefs_of_card(&mut *tx, id).await?;
    repo::tombstone_card(&mut *tx, id, now, seq).await?;
    tx.commit().await?;
    Ok(())
}

pub async fn set_favorite(
    pool: &SqlitePool,
    viewer: Viewer,
    id: Uuid,
    is_favorite: bool,
) -> Result<CardDto, ApiError> {
    let mut tx = pool.begin().await?;
    let seq = repo::next_change_seq(&mut *tx).await?;
    require_visible(&mut tx, viewer, id).await?;
    repo::set_favorite(&mut *tx, id, viewer.user_id, is_favorite, seq).await?;
    let card = load_dto(&mut tx, viewer, id).await?;
    tx.commit().await?;
    Ok(card)
}

/// Counts an opening of the card, for the "recently used" ordering.
pub async fn record_use(
    pool: &SqlitePool,
    viewer: Viewer,
    id: Uuid,
    now: i64,
) -> Result<(), ApiError> {
    let mut tx = pool.begin().await?;
    let seq = repo::next_change_seq(&mut *tx).await?;
    require_visible(&mut tx, viewer, id).await?;
    repo::record_use(&mut *tx, id, viewer.user_id, now, seq).await?;
    tx.commit().await?;
    Ok(())
}

pub async fn put_image(
    pool: &SqlitePool,
    viewer: Viewer,
    card_id: Uuid,
    kind: ImageKind,
    mime: ImageMime,
    data: &[u8],
    now: i64,
) -> Result<Uuid, ApiError> {
    let image_id = Uuid::now_v7();
    let mut tx = pool.begin().await?;
    let seq = repo::next_change_seq(&mut *tx).await?;
    let card = require_editable(&mut tx, viewer, card_id).await?;
    let stored = repo::image_bytes_of(&mut *tx, card.owner_id, (card_id, kind)).await?;
    let incoming = i64::try_from(data.len()).unwrap_or(i64::MAX);
    if stored.saturating_add(incoming) > MAX_IMAGE_BYTES_PER_USER {
        return Err(ApiError::QuotaExceeded(
            "photo storage is full; remove some photos first",
        ));
    }
    repo::replace_image(&mut *tx, image_id, card_id, kind, mime.as_str(), data, now).await?;
    repo::touch_card(&mut *tx, card_id, now, seq).await?;
    tx.commit().await?;
    Ok(image_id)
}

/// Removes a photo; removing a photo that is not there succeeds.
pub async fn delete_image(
    pool: &SqlitePool,
    viewer: Viewer,
    card_id: Uuid,
    kind: ImageKind,
    now: i64,
) -> Result<(), ApiError> {
    let mut tx = pool.begin().await?;
    let seq = repo::next_change_seq(&mut *tx).await?;
    require_editable(&mut tx, viewer, card_id).await?;
    if repo::delete_image(&mut *tx, card_id, kind).await? {
        repo::touch_card(&mut *tx, card_id, now, seq).await?;
    }
    tx.commit().await?;
    Ok(())
}

pub async fn get_image(
    pool: &SqlitePool,
    viewer: Viewer,
    image_id: Uuid,
) -> Result<StoredImage, ApiError> {
    let image = repo::find_image(pool, image_id)
        .await?
        .ok_or(ApiError::NotFound)?;
    let access = Ownership {
        owner_id: image.owner_id,
        household_id: image.household_id,
        deleted: false,
    };
    if policy::can_view(viewer, access) {
        Ok(image)
    } else {
        Err(ApiError::NotFound)
    }
}

// ---------- helpers ----------

fn ownership(card: &CardAccess) -> Ownership {
    Ownership {
        owner_id: card.owner_id,
        household_id: card.household_id,
        deleted: card.deleted_at.is_some(),
    }
}

async fn require_visible(
    conn: &mut SqliteConnection,
    viewer: Viewer,
    id: Uuid,
) -> Result<(), ApiError> {
    let card = repo::find_access(&mut *conn, id).await?;
    match card {
        Some(card) if policy::can_view(viewer, ownership(&card)) => Ok(()),
        _ => Err(ApiError::NotFound),
    }
}

async fn require_editable(
    conn: &mut SqliteConnection,
    viewer: Viewer,
    id: Uuid,
) -> Result<CardAccess, ApiError> {
    let card = repo::find_access(&mut *conn, id).await?;
    match card {
        Some(card) if policy::can_edit(viewer, ownership(&card)) => Ok(card),
        _ => Err(ApiError::NotFound),
    }
}

async fn load_dto(
    conn: &mut SqliteConnection,
    viewer: Viewer,
    id: Uuid,
) -> Result<CardDto, ApiError> {
    repo::find_dto(conn, viewer.user_id, id)
        .await?
        .ok_or_else(|| ApiError::Internal(format!("card {id} vanished inside its transaction")))
}

/// Trims and validates every field of a card.
pub fn validate(input: CardInput) -> Result<CardFields, ApiError> {
    let barcode_format: BarcodeFormat = input.barcode_format;
    Ok(CardFields {
        store_name: required_text("store name", &input.store_name, MAX_STORE_NAME_CHARS)?,
        card_number: barcode_format.normalize(&input.card_number)?,
        barcode_format,
        member_name: optional_text("member name", input.member_name, MAX_MEMBER_NAME_CHARS)?,
        color: input.color.map(|c| validate_color(&c)).transpose()?,
        notes: optional_multiline("notes", input.notes, MAX_NOTES_CHARS)?,
    })
}

fn required_text(field: &str, raw: &str, max: usize) -> Result<String, ApiError> {
    let value = raw.trim();
    let length = value.chars().count();
    if length == 0 || length > max {
        return Err(ApiError::Validation(format!(
            "{field} must be between 1 and {max} characters"
        )));
    }
    if value.chars().any(char::is_control) {
        return Err(ApiError::Validation(format!(
            "{field} contains invalid characters"
        )));
    }
    Ok(value.to_owned())
}

/// Blank optional fields are stored as NULL.
fn optional_text(field: &str, raw: Option<String>, max: usize) -> Result<Option<String>, ApiError> {
    match raw.as_deref().map(str::trim) {
        None | Some("") => Ok(None),
        Some(value) => required_text(field, value, max).map(Some),
    }
}

fn optional_multiline(
    field: &str,
    raw: Option<String>,
    max: usize,
) -> Result<Option<String>, ApiError> {
    let Some(value) = raw.as_deref().map(str::trim).filter(|v| !v.is_empty()) else {
        return Ok(None);
    };
    if value.chars().count() > max {
        return Err(ApiError::Validation(format!(
            "{field} must be at most {max} characters"
        )));
    }
    if value
        .chars()
        .any(|c| c.is_control() && c != '\n' && c != '\t')
    {
        return Err(ApiError::Validation(format!(
            "{field} contains invalid characters"
        )));
    }
    Ok(Some(value.to_owned()))
}

/// `#rrggbb`, stored lower-case.
fn validate_color(raw: &str) -> Result<String, ApiError> {
    let color = raw.trim().to_ascii_lowercase();
    let valid = color.len() == 7
        && color.starts_with('#')
        && color[1..].bytes().all(|b| b.is_ascii_hexdigit());
    if valid {
        Ok(color)
    } else {
        Err(ApiError::Validation("color must look like #12ab34".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input() -> CardInput {
        CardInput {
            store_name: "  Esselunga ".into(),
            card_number: "4006381333931".into(),
            barcode_format: BarcodeFormat::Ean13,
            member_name: Some("  ".into()),
            color: Some("#AABBCC".into()),
            notes: Some(" line 1\nline 2 ".into()),
        }
    }

    #[test]
    fn validate_trims_and_normalises_fields() {
        let fields = validate(input()).unwrap();

        assert_eq!(fields.store_name, "Esselunga");
        assert_eq!(fields.member_name, None);
        assert_eq!(fields.color.as_deref(), Some("#aabbcc"));
        assert_eq!(fields.notes.as_deref(), Some("line 1\nline 2"));
    }

    #[test]
    fn validate_rejects_bad_fields() {
        let cases = [
            (
                CardInput {
                    store_name: " ".into(),
                    ..input()
                },
                "store name",
            ),
            (
                CardInput {
                    store_name: "x".repeat(65),
                    ..input()
                },
                "store name",
            ),
            (
                CardInput {
                    store_name: "a\u{7}b".into(),
                    ..input()
                },
                "invalid characters",
            ),
            (
                CardInput {
                    card_number: "123".into(),
                    ..input()
                },
                "13 digits",
            ),
            (
                CardInput {
                    member_name: Some("m".repeat(65)),
                    ..input()
                },
                "member name",
            ),
            (
                CardInput {
                    color: Some("red".into()),
                    ..input()
                },
                "color",
            ),
            (
                CardInput {
                    color: Some("#12345g".into()),
                    ..input()
                },
                "color",
            ),
            (
                CardInput {
                    notes: Some("n".repeat(1001)),
                    ..input()
                },
                "notes",
            ),
            (
                CardInput {
                    notes: Some("a\u{0}".into()),
                    ..input()
                },
                "invalid characters",
            ),
        ];

        for (card, expected) in cases {
            let message = validate(card).unwrap_err().to_string();
            assert!(
                message.contains(expected),
                "{message:?} should mention {expected:?}"
            );
        }
    }
}
