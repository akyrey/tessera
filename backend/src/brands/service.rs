//! Brand search, linking and store-location refresh.

use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use uuid::Uuid;

use super::model::{BrandDto, BrandLocationsDto, BrandSearchDto, LocationsDto};
use super::repo;
use super::upstream::Upstream;
use super::{overpass, wikidata};
use crate::accounts::repo as accounts_repo;
use crate::cards::repo as cards_repo;
use crate::error::ApiError;

/// Searches shorter than this are too vague for a useful Wikidata lookup.
const MIN_QUERY_CHARS: usize = 2;
const MAX_QUERY_CHARS: usize = 64;
const LOCAL_RESULTS: u32 = 5;
/// Far beyond two people's wallets; stops anyone from filling the brand table.
pub const MAX_BRANDS: i64 = 1_000;

pub async fn search(
    pool: &SqlitePool,
    upstream: &Upstream,
    user_id: Uuid,
    raw_query: &str,
) -> Result<BrandSearchDto, ApiError> {
    let query = raw_query.trim();
    let length = query.chars().count();
    if !(MIN_QUERY_CHARS..=MAX_QUERY_CHARS).contains(&length) || query.chars().any(char::is_control)
    {
        return Err(ApiError::Validation(format!(
            "search for {MIN_QUERY_CHARS} to {MAX_QUERY_CHARS} characters"
        )));
    }
    let household_id = accounts_repo::membership_of(pool, user_id)
        .await?
        .map(|m| m.household_id);
    let linked = repo::search(pool, user_id, household_id, query, LOCAL_RESULTS).await?;
    let (candidates, remote_unavailable) = match upstream.search_brands(query).await {
        Ok(hits) => (hits, false),
        Err(error) => {
            tracing::warn!(%error, "Wikidata search failed");
            (Vec::new(), true)
        }
    };
    let candidates = candidates
        .into_iter()
        .filter(|hit| {
            !linked
                .iter()
                .any(|brand| brand.wikidata_id == hit.wikidata_id)
        })
        .collect();
    Ok(BrandSearchDto {
        linked,
        candidates,
        remote_unavailable,
    })
}

/// Creates the brand for a Wikidata item (its name comes from Wikidata, not
/// from the client) and returns it. Linking an existing brand is a no-op.
pub async fn link(
    pool: &SqlitePool,
    upstream: &Upstream,
    qid: &str,
    now: i64,
) -> Result<(BrandDto, bool), ApiError> {
    if !wikidata::is_qid(qid) {
        return Err(ApiError::Validation("not a Wikidata item id".into()));
    }
    if let Some(brand) = repo::find_by_wikidata(pool, qid).await? {
        return Ok((brand, false));
    }
    if repo::count(pool).await? >= MAX_BRANDS {
        return Err(ApiError::QuotaExceeded("too many brands on this server"));
    }
    let name = upstream
        .brand_label(qid)
        .await
        .map_err(|error| {
            tracing::warn!(%error, qid, "Wikidata label lookup failed");
            ApiError::Unavailable
        })?
        .ok_or_else(|| ApiError::Validation("Wikidata has no such item".into()))?;
    repo::insert_if_missing(pool, qid, &name, now).await?;
    let brand = repo::find_by_wikidata(pool, qid)
        .await?
        .ok_or_else(|| ApiError::Internal(format!("brand {qid} vanished after insert")))?;
    Ok((brand, true))
}

/// Replaces the brand's shops with a fresh Overpass result. A failed, partial
/// or suspiciously empty answer keeps the shops already stored.
pub async fn refresh_locations(
    pool: &SqlitePool,
    upstream: &Upstream,
    brand_id: Uuid,
    qid: &str,
    now: i64,
) -> Result<usize, ApiError> {
    // Stored ids were validated on insert; re-check before building a query from one.
    if !wikidata::is_qid(qid) {
        return Err(ApiError::Internal(format!(
            "brand {brand_id} has an invalid Wikidata id"
        )));
    }
    let fetched = upstream.store_locations(qid).await;
    let locations = match fetched {
        Ok(locations)
            if !locations.is_empty() || repo::count_locations(pool, brand_id).await? == 0 =>
        {
            locations
        }
        Ok(_) => {
            tracing::warn!(qid, "Overpass returned no shops; keeping the stored ones");
            repo::mark_failed(pool, brand_id, now).await?;
            return Err(ApiError::Unavailable);
        }
        Err(error) => {
            tracing::warn!(%error, qid, "Overpass query failed");
            repo::mark_failed(pool, brand_id, now).await?;
            return Err(ApiError::Unavailable);
        }
    };
    if locations.len() >= overpass::MAX_LOCATIONS {
        tracing::warn!(
            qid,
            "store locations truncated at {}",
            overpass::MAX_LOCATIONS
        );
    }
    let mut tx = pool.begin().await?;
    repo::delete_locations(&mut *tx, brand_id).await?;
    repo::insert_locations(&mut tx, brand_id, &locations).await?;
    repo::mark_synced(&mut *tx, brand_id, now).await?;
    tx.commit().await?;
    tracing::info!(qid, count = locations.len(), "store locations refreshed");
    Ok(locations.len())
}

/// Looks up the brand's logo and stores it when it changed. Returns whether
/// the stored logo changed. No logo on Wikidata keeps any logo already stored.
pub async fn refresh_logo(
    pool: &SqlitePool,
    upstream: &Upstream,
    brand_id: Uuid,
    qid: &str,
    now: i64,
) -> Result<bool, ApiError> {
    if !wikidata::is_qid(qid) {
        return Err(ApiError::Internal(format!(
            "brand {brand_id} has an invalid Wikidata id"
        )));
    }
    let logo = match upstream.brand_logo(qid).await {
        Ok(Some(logo)) => logo,
        Ok(None) => {
            repo::mark_logo_checked(pool, brand_id, now).await?;
            return Ok(false);
        }
        Err(error) => {
            tracing::warn!(%error, qid, "brand logo lookup failed");
            repo::mark_logo_failed(pool, brand_id, now).await?;
            return Err(ApiError::Unavailable);
        }
    };
    // Only this job writes logos, so comparing outside the transaction is safe.
    let unchanged = repo::logo_data_of(pool, brand_id).await?.as_deref() == Some(&logo.data[..]);
    if unchanged {
        repo::mark_logo_checked(pool, brand_id, now).await?;
        return Ok(false);
    }
    let mut tx = pool.begin().await?;
    // As the first write it takes the write lock; a new cursor also makes every
    // client pull a fresh snapshot, which carries the new logo id.
    cards_repo::next_change_seq(&mut *tx).await?;
    repo::replace_logo(&mut *tx, brand_id, logo.mime.as_str(), &logo.data, now).await?;
    repo::mark_logo_checked(&mut *tx, brand_id, now).await?;
    tx.commit().await?;
    tracing::info!(qid, bytes = logo.data.len(), "brand logo stored");
    Ok(true)
}

/// A stored logo: `(mime, bytes)`. Logos are public brand imagery, so any
/// signed-in user may read any of them.
pub async fn get_logo(pool: &SqlitePool, logo_id: Uuid) -> Result<(String, Vec<u8>), ApiError> {
    repo::find_logo(pool, logo_id)
        .await?
        .ok_or(ApiError::NotFound)
}

/// Store locations for every brand on the caller's visible cards.
pub async fn locations_for(
    pool: &SqlitePool,
    user_id: Uuid,
    known_version: Option<&str>,
    radius_m: u32,
) -> Result<LocationsDto, ApiError> {
    let mut tx = pool.begin().await?;
    let household_id = accounts_repo::membership_of(&mut *tx, user_id)
        .await?
        .map(|m| m.household_id);
    let brands = repo::visible_brands(&mut *tx, user_id, household_id).await?;
    let version = version_of(&brands, radius_m);
    if known_version == Some(version.as_str()) {
        return Ok(LocationsDto {
            version,
            radius_m,
            brands: None,
        });
    }
    let mut result = Vec::with_capacity(brands.len());
    for (brand_id, _) in brands {
        let points = repo::points_of(&mut *tx, brand_id).await?;
        result.push(BrandLocationsDto { brand_id, points });
    }
    tx.commit().await?;
    Ok(LocationsDto {
        version,
        radius_m,
        brands: Some(result),
    })
}

/// Changes whenever the set of brands, one of their refreshes, or the radius changes.
fn version_of(brands: &[(Uuid, Option<i64>)], radius_m: u32) -> String {
    let mut hasher = Sha256::new();
    hasher.update(radius_m.to_le_bytes());
    for (id, synced) in brands {
        hasher.update(id.as_bytes());
        hasher.update(synced.unwrap_or(-1).to_le_bytes());
    }
    let digest = hasher.finalize();
    digest[..12].iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::version_of;
    use uuid::Uuid;

    #[test]
    fn version_tracks_brands_refreshes_and_radius() {
        let a = (Uuid::from_u128(1), Some(10));
        let base = version_of(&[a], 200);

        assert_eq!(base, version_of(&[a], 200));
        assert_ne!(base, version_of(&[(a.0, Some(11))], 200));
        assert_ne!(base, version_of(&[a, (Uuid::from_u128(2), None)], 200));
        assert_ne!(base, version_of(&[a], 300));
        assert_eq!(base.len(), 24);
    }
}
