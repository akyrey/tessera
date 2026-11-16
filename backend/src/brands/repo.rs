//! SQL for brands and store locations.

use sqlx::SqliteExecutor;
use uuid::Uuid;

use super::model::BrandDto;
use super::overpass::StoreLocation;

const BRAND_SELECT: &str = "
    SELECT b.id, b.wikidata_id, b.name, b.last_synced_at,
           (SELECT COUNT(*) FROM store_locations l WHERE l.brand_id = b.id) AS location_count
    FROM brands b";

pub async fn find_by_wikidata(
    ex: impl SqliteExecutor<'_>,
    qid: &str,
) -> sqlx::Result<Option<BrandDto>> {
    let sql = format!("{BRAND_SELECT} WHERE b.wikidata_id = ?");
    sqlx::query_as(sqlx::AssertSqlSafe(sql))
        .bind(qid)
        .fetch_optional(ex)
        .await
}

pub async fn find(ex: impl SqliteExecutor<'_>, id: Uuid) -> sqlx::Result<Option<BrandDto>> {
    let sql = format!("{BRAND_SELECT} WHERE b.id = ?");
    sqlx::query_as(sqlx::AssertSqlSafe(sql))
        .bind(id)
        .fetch_optional(ex)
        .await
}

/// Brands on cards the user can see whose name contains `query`
/// (case-insensitive for ASCII). Other households' brands stay private.
pub async fn search(
    ex: impl SqliteExecutor<'_>,
    user_id: Uuid,
    household_id: Option<Uuid>,
    query: &str,
    limit: u32,
) -> sqlx::Result<Vec<BrandDto>> {
    let pattern = format!("%{}%", escape_like(query));
    let sql = format!(
        "{BRAND_SELECT}
         WHERE b.name LIKE ?1 ESCAPE '\\'
           AND EXISTS (SELECT 1 FROM cards c WHERE c.brand_id = b.id AND c.deleted_at IS NULL
                       AND (c.owner_id = ?2 OR c.household_id = ?3))
         ORDER BY b.name LIMIT ?4"
    );
    sqlx::query_as(sqlx::AssertSqlSafe(sql))
        .bind(pattern)
        .bind(user_id)
        .bind(household_id)
        .bind(limit)
        .fetch_all(ex)
        .await
}

pub async fn count(ex: impl SqliteExecutor<'_>) -> sqlx::Result<i64> {
    sqlx::query_scalar("SELECT COUNT(*) FROM brands")
        .fetch_one(ex)
        .await
}

pub async fn count_locations(ex: impl SqliteExecutor<'_>, brand_id: Uuid) -> sqlx::Result<i64> {
    sqlx::query_scalar("SELECT COUNT(*) FROM store_locations WHERE brand_id = ?")
        .bind(brand_id)
        .fetch_one(ex)
        .await
}

/// Wikidata id and last sync of a brand.
pub async fn sync_state(
    ex: impl SqliteExecutor<'_>,
    brand_id: Uuid,
) -> sqlx::Result<Option<(String, Option<i64>)>> {
    sqlx::query_as("SELECT wikidata_id, last_synced_at FROM brands WHERE id = ?")
        .bind(brand_id)
        .fetch_optional(ex)
        .await
}

fn escape_like(text: &str) -> String {
    text.chars()
        .flat_map(|c| match c {
            '%' | '_' | '\\' => vec!['\\', c],
            _ => vec![c],
        })
        .collect()
}

/// Inserts the brand unless one with this Wikidata id exists.
pub async fn insert_if_missing(
    ex: impl SqliteExecutor<'_>,
    qid: &str,
    name: &str,
    now: i64,
) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO brands (id, wikidata_id, name, created_at) VALUES (?, ?, ?, ?)
         ON CONFLICT (wikidata_id) DO NOTHING",
    )
    .bind(Uuid::now_v7())
    .bind(qid)
    .bind(name)
    .bind(now)
    .execute(ex)
    .await
    .map(|_| ())
}

pub async fn exists(ex: impl SqliteExecutor<'_>, id: Uuid) -> sqlx::Result<bool> {
    sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM brands WHERE id = ?)")
        .bind(id)
        .fetch_one(ex)
        .await
}

/// Brands used by a live card whose locations are missing or older than
/// `stale_before`, skipping those that failed after `retry_after_error_before`.
pub async fn due_for_refresh(
    ex: impl SqliteExecutor<'_>,
    stale_before: i64,
    retry_after_error_before: i64,
) -> sqlx::Result<Vec<(Uuid, String)>> {
    sqlx::query_as(
        "SELECT b.id, b.wikidata_id FROM brands b
         WHERE EXISTS (SELECT 1 FROM cards c WHERE c.brand_id = b.id AND c.deleted_at IS NULL)
           AND (b.last_synced_at IS NULL OR b.last_synced_at < ?)
           AND (b.last_error_at IS NULL OR b.last_error_at < ?)
         ORDER BY b.last_synced_at IS NOT NULL, b.last_synced_at",
    )
    .bind(stale_before)
    .bind(retry_after_error_before)
    .fetch_all(ex)
    .await
}

pub async fn delete_locations(ex: impl SqliteExecutor<'_>, brand_id: Uuid) -> sqlx::Result<()> {
    sqlx::query("DELETE FROM store_locations WHERE brand_id = ?")
        .bind(brand_id)
        .execute(ex)
        .await
        .map(|_| ())
}

/// Rows per INSERT: keeps statements well under SQLite's parameter limit.
const INSERT_BATCH: usize = 500;

pub async fn insert_locations(
    conn: &mut sqlx::SqliteConnection,
    brand_id: Uuid,
    locations: &[StoreLocation],
) -> sqlx::Result<()> {
    for batch in locations.chunks(INSERT_BATCH) {
        let mut builder = sqlx::QueryBuilder::new(
            "INSERT OR REPLACE INTO store_locations (brand_id, osm_type, osm_id, lat, lon, name) ",
        );
        builder.push_values(batch, |mut row, location| {
            row.push_bind(brand_id)
                .push_bind(location.osm_type)
                .push_bind(location.osm_id)
                .push_bind(location.lat)
                .push_bind(location.lon)
                .push_bind(location.name.clone());
        });
        builder.build().execute(&mut *conn).await?;
    }
    Ok(())
}

pub async fn mark_synced(
    ex: impl SqliteExecutor<'_>,
    brand_id: Uuid,
    now: i64,
) -> sqlx::Result<()> {
    sqlx::query("UPDATE brands SET last_synced_at = ?, last_error_at = NULL WHERE id = ?")
        .bind(now)
        .bind(brand_id)
        .execute(ex)
        .await
        .map(|_| ())
}

pub async fn mark_failed(
    ex: impl SqliteExecutor<'_>,
    brand_id: Uuid,
    now: i64,
) -> sqlx::Result<()> {
    sqlx::query("UPDATE brands SET last_error_at = ? WHERE id = ?")
        .bind(now)
        .bind(brand_id)
        .execute(ex)
        .await
        .map(|_| ())
}

/// `(brand id, last sync)` of every brand linked to a card visible to the user.
pub async fn visible_brands(
    ex: impl SqliteExecutor<'_>,
    user_id: Uuid,
    household_id: Option<Uuid>,
) -> sqlx::Result<Vec<(Uuid, Option<i64>)>> {
    sqlx::query_as(
        "SELECT DISTINCT b.id, b.last_synced_at FROM brands b
         JOIN cards c ON c.brand_id = b.id
         WHERE c.deleted_at IS NULL AND (c.owner_id = ? OR c.household_id = ?)
         ORDER BY b.id",
    )
    .bind(user_id)
    .bind(household_id)
    .fetch_all(ex)
    .await
}

pub async fn points_of(
    ex: impl SqliteExecutor<'_>,
    brand_id: Uuid,
) -> sqlx::Result<Vec<(f64, f64)>> {
    sqlx::query_as(
        "SELECT lat, lon FROM store_locations WHERE brand_id = ? ORDER BY osm_type, osm_id",
    )
    .bind(brand_id)
    .fetch_all(ex)
    .await
}

#[cfg(test)]
mod tests {
    use super::escape_like;

    #[test]
    fn like_wildcards_are_escaped() {
        assert_eq!(escape_like("50%_off\\"), "50\\%\\_off\\\\");
        assert_eq!(escape_like("Coop"), "Coop");
    }
}
