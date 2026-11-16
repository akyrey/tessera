//! SQL for cards, per-user preferences and photos.
//!
//! Every function runs a single statement, so it accepts either the pool or
//! a transaction (`&mut *tx`).

use sqlx::SqliteExecutor;
use uuid::Uuid;

use super::model::{CardAccess, CardDto, CardFields, ImageKind, Place, StoredImage};

/// Columns of `CardDto` for the user bound as the first parameter.
const CARD_SELECT: &str = "
    SELECT c.id, c.owner_id,
           (SELECT u.display_name FROM users u WHERE u.id = c.owner_id) AS owner_name,
           c.household_id IS NOT NULL AS shared,
           c.store_name, c.card_number, c.barcode_format,
           c.member_name, c.color, c.notes,
           COALESCE(p.is_favorite, 0) AS is_favorite,
           COALESCE(p.use_count, 0) AS use_count,
           p.last_used_at,
           (SELECT i.id FROM card_images i WHERE i.card_id = c.id AND i.kind = 'front') AS front_image_id,
           (SELECT i.id FROM card_images i WHERE i.card_id = c.id AND i.kind = 'back') AS back_image_id,
           c.brand_id,
           (SELECT b.name FROM brands b WHERE b.id = c.brand_id) AS brand_name,
           c.expires_on,
           c.created_at, c.updated_at
    FROM cards c
    LEFT JOIN card_user_prefs p ON p.card_id = c.id AND p.user_id = ?1";

/// Takes the next change number. As the first write of a transaction it also
/// takes SQLite's write lock up front, so concurrent writers queue instead of
/// failing on a read-to-write lock upgrade.
pub async fn next_change_seq(ex: impl SqliteExecutor<'_>) -> sqlx::Result<i64> {
    sqlx::query_scalar("UPDATE change_counter SET value = value + 1 WHERE id = 1 RETURNING value")
        .fetch_one(ex)
        .await
}

pub async fn current_change_seq(ex: impl SqliteExecutor<'_>) -> sqlx::Result<i64> {
    sqlx::query_scalar("SELECT value FROM change_counter WHERE id = 1")
        .fetch_one(ex)
        .await
}

// ---------- cards ----------

/// Cards visible to `user_id` (own cards and cards shared with `household_id`).
pub async fn list_visible(
    ex: impl SqliteExecutor<'_>,
    user_id: Uuid,
    household_id: Option<Uuid>,
) -> sqlx::Result<Vec<CardDto>> {
    let sql = format!(
        "{CARD_SELECT}
         WHERE c.deleted_at IS NULL AND (c.owner_id = ?1 OR c.household_id = ?2)
         ORDER BY c.store_name COLLATE NOCASE, c.id"
    );
    sqlx::query_as(sqlx::AssertSqlSafe(sql))
        .bind(user_id)
        .bind(household_id)
        .fetch_all(ex)
        .await
}

/// One card with `user_id`'s preferences; the caller checks access first.
pub async fn find_dto(
    ex: impl SqliteExecutor<'_>,
    user_id: Uuid,
    card_id: Uuid,
) -> sqlx::Result<Option<CardDto>> {
    let sql = format!("{CARD_SELECT} WHERE c.id = ?2");
    sqlx::query_as(sqlx::AssertSqlSafe(sql))
        .bind(user_id)
        .bind(card_id)
        .fetch_optional(ex)
        .await
}

pub async fn find_access(
    ex: impl SqliteExecutor<'_>,
    card_id: Uuid,
) -> sqlx::Result<Option<CardAccess>> {
    sqlx::query_as("SELECT id, owner_id, household_id, deleted_at FROM cards WHERE id = ?")
        .bind(card_id)
        .fetch_optional(ex)
        .await
}

pub async fn insert_card(
    ex: impl SqliteExecutor<'_>,
    id: Uuid,
    owner_id: Uuid,
    fields: &CardFields,
    now: i64,
    seq: i64,
) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO cards (id, owner_id, store_name, card_number, barcode_format,
                            member_name, color, notes, brand_id, expires_on,
                            created_at, updated_at, change_seq)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(id)
    .bind(owner_id)
    .bind(&fields.store_name)
    .bind(&fields.card_number)
    .bind(fields.barcode_format)
    .bind(&fields.member_name)
    .bind(&fields.color)
    .bind(&fields.notes)
    .bind(fields.brand_id)
    .bind(&fields.expires_on)
    .bind(now)
    .bind(now)
    .bind(seq)
    .execute(ex)
    .await
    .map(|_| ())
}

pub async fn update_card(
    ex: impl SqliteExecutor<'_>,
    id: Uuid,
    fields: &CardFields,
    now: i64,
    seq: i64,
) -> sqlx::Result<()> {
    sqlx::query(
        "UPDATE cards SET store_name = ?, card_number = ?, barcode_format = ?,
                          member_name = ?, color = ?, notes = ?, brand_id = ?, expires_on = ?,
                          updated_at = ?, change_seq = ?
         WHERE id = ?",
    )
    .bind(&fields.store_name)
    .bind(&fields.card_number)
    .bind(fields.barcode_format)
    .bind(&fields.member_name)
    .bind(&fields.color)
    .bind(&fields.notes)
    .bind(fields.brand_id)
    .bind(&fields.expires_on)
    .bind(now)
    .bind(seq)
    .bind(id)
    .execute(ex)
    .await
    .map(|_| ())
}

pub async fn set_household(
    ex: impl SqliteExecutor<'_>,
    id: Uuid,
    household_id: Option<Uuid>,
    now: i64,
    seq: i64,
) -> sqlx::Result<()> {
    sqlx::query("UPDATE cards SET household_id = ?, updated_at = ?, change_seq = ? WHERE id = ?")
        .bind(household_id)
        .bind(now)
        .bind(seq)
        .bind(id)
        .execute(ex)
        .await
        .map(|_| ())
}

/// Marks a card as changed without touching its fields (e.g. a new photo).
pub async fn touch_card(
    ex: impl SqliteExecutor<'_>,
    id: Uuid,
    now: i64,
    seq: i64,
) -> sqlx::Result<()> {
    sqlx::query("UPDATE cards SET updated_at = ?, change_seq = ? WHERE id = ?")
        .bind(now)
        .bind(seq)
        .bind(id)
        .execute(ex)
        .await
        .map(|_| ())
}

/// Leaves a tombstone (id, owner, `deleted_at`) for sync and erases the content.
pub async fn tombstone_card(
    ex: impl SqliteExecutor<'_>,
    id: Uuid,
    now: i64,
    seq: i64,
) -> sqlx::Result<()> {
    sqlx::query(
        "UPDATE cards SET deleted_at = ?, updated_at = ?, change_seq = ?,
                          store_name = '-', card_number = '-', barcode_format = 'none',
                          member_name = NULL, color = NULL, notes = NULL, household_id = NULL,
                          brand_id = NULL, expires_on = NULL
         WHERE id = ?",
    )
    .bind(now)
    .bind(now)
    .bind(seq)
    .bind(id)
    .execute(ex)
    .await
    .map(|_| ())
}

// ---------- places ----------

pub async fn replace_places(
    conn: &mut sqlx::SqliteConnection,
    card_id: Uuid,
    places: &[Place],
) -> sqlx::Result<()> {
    sqlx::query("DELETE FROM card_places WHERE card_id = ?")
        .bind(card_id)
        .execute(&mut *conn)
        .await?;
    for (position, place) in places.iter().enumerate() {
        sqlx::query(
            "INSERT INTO card_places (card_id, position, label, lat, lon) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(card_id)
        .bind(i64::try_from(position).unwrap_or(i64::MAX))
        .bind(&place.label)
        .bind(place.lat)
        .bind(place.lon)
        .execute(&mut *conn)
        .await?;
    }
    Ok(())
}

/// Places of the cards visible to the user, keyed by card id.
pub async fn visible_places(
    ex: impl SqliteExecutor<'_>,
    user_id: Uuid,
    household_id: Option<Uuid>,
) -> sqlx::Result<Vec<(Uuid, String, f64, f64)>> {
    sqlx::query_as(
        "SELECT p.card_id, p.label, p.lat, p.lon FROM card_places p
         JOIN cards c ON c.id = p.card_id
         WHERE c.deleted_at IS NULL AND (c.owner_id = ?1 OR c.household_id = ?2)
         ORDER BY p.card_id, p.position",
    )
    .bind(user_id)
    .bind(household_id)
    .fetch_all(ex)
    .await
}

pub async fn places_of(ex: impl SqliteExecutor<'_>, card_id: Uuid) -> sqlx::Result<Vec<Place>> {
    let rows: Vec<(String, f64, f64)> = sqlx::query_as(
        "SELECT label, lat, lon FROM card_places WHERE card_id = ? ORDER BY position",
    )
    .bind(card_id)
    .fetch_all(ex)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(label, lat, lon)| Place { label, lat, lon })
        .collect())
}

pub async fn delete_places_of_card(ex: impl SqliteExecutor<'_>, card_id: Uuid) -> sqlx::Result<()> {
    sqlx::query("DELETE FROM card_places WHERE card_id = ?")
        .bind(card_id)
        .execute(ex)
        .await
        .map(|_| ())
}

// ---------- preferences ----------

pub async fn set_favorite(
    ex: impl SqliteExecutor<'_>,
    card_id: Uuid,
    user_id: Uuid,
    is_favorite: bool,
    seq: i64,
) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO card_user_prefs (card_id, user_id, is_favorite, change_seq)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT (card_id, user_id)
         DO UPDATE SET is_favorite = ?3, change_seq = ?4",
    )
    .bind(card_id)
    .bind(user_id)
    .bind(is_favorite)
    .bind(seq)
    .execute(ex)
    .await
    .map(|_| ())
}

pub async fn record_use(
    ex: impl SqliteExecutor<'_>,
    card_id: Uuid,
    user_id: Uuid,
    now: i64,
    seq: i64,
) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO card_user_prefs (card_id, user_id, use_count, last_used_at, change_seq)
         VALUES (?1, ?2, 1, ?3, ?4)
         ON CONFLICT (card_id, user_id)
         DO UPDATE SET use_count = use_count + 1, last_used_at = ?3, change_seq = ?4",
    )
    .bind(card_id)
    .bind(user_id)
    .bind(now)
    .bind(seq)
    .execute(ex)
    .await
    .map(|_| ())
}

pub async fn delete_prefs_of_card(ex: impl SqliteExecutor<'_>, card_id: Uuid) -> sqlx::Result<()> {
    sqlx::query("DELETE FROM card_user_prefs WHERE card_id = ?")
        .bind(card_id)
        .execute(ex)
        .await
        .map(|_| ())
}

// ---------- images ----------

pub async fn replace_image(
    ex: impl SqliteExecutor<'_>,
    id: Uuid,
    card_id: Uuid,
    kind: ImageKind,
    mime: &str,
    data: &[u8],
    now: i64,
) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO card_images (id, card_id, kind, mime, data, created_at)
         VALUES (?, ?, ?, ?, ?, ?)
         ON CONFLICT (card_id, kind)
         DO UPDATE SET id = excluded.id, mime = excluded.mime, data = excluded.data,
                       created_at = excluded.created_at",
    )
    .bind(id)
    .bind(card_id)
    .bind(kind)
    .bind(mime)
    .bind(data)
    .bind(now)
    .execute(ex)
    .await
    .map(|_| ())
}

/// Returns whether a photo was deleted.
pub async fn delete_image(
    ex: impl SqliteExecutor<'_>,
    card_id: Uuid,
    kind: ImageKind,
) -> sqlx::Result<bool> {
    sqlx::query("DELETE FROM card_images WHERE card_id = ? AND kind = ?")
        .bind(card_id)
        .bind(kind)
        .execute(ex)
        .await
        .map(|result| result.rows_affected() > 0)
}

pub async fn delete_images_of_card(ex: impl SqliteExecutor<'_>, card_id: Uuid) -> sqlx::Result<()> {
    sqlx::query("DELETE FROM card_images WHERE card_id = ?")
        .bind(card_id)
        .execute(ex)
        .await
        .map(|_| ())
}

/// A photo of a card that is not deleted, with the card's ownership.
pub async fn find_image(
    ex: impl SqliteExecutor<'_>,
    image_id: Uuid,
) -> sqlx::Result<Option<StoredImage>> {
    sqlx::query_as(
        "SELECT i.mime, i.data, c.owner_id, c.household_id
         FROM card_images i JOIN cards c ON c.id = i.card_id
         WHERE i.id = ? AND c.deleted_at IS NULL",
    )
    .bind(image_id)
    .fetch_optional(ex)
    .await
}

// ---------- quotas ----------

pub async fn count_live_cards_of(ex: impl SqliteExecutor<'_>, owner_id: Uuid) -> sqlx::Result<i64> {
    sqlx::query_scalar("SELECT COUNT(*) FROM cards WHERE owner_id = ? AND deleted_at IS NULL")
        .bind(owner_id)
        .fetch_one(ex)
        .await
}

/// Bytes of photos on cards owned by `owner_id`, excluding `except` (being replaced).
pub async fn image_bytes_of(
    ex: impl SqliteExecutor<'_>,
    owner_id: Uuid,
    except: (Uuid, ImageKind),
) -> sqlx::Result<i64> {
    sqlx::query_scalar(
        "SELECT COALESCE(SUM(length(i.data)), 0)
         FROM card_images i JOIN cards c ON c.id = i.card_id
         WHERE c.owner_id = ? AND NOT (i.card_id = ? AND i.kind = ?)",
    )
    .bind(owner_id)
    .bind(except.0)
    .bind(except.1)
    .fetch_one(ex)
    .await
}
