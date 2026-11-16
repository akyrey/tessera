//! Brand search, linking and store-location refresh.

use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use uuid::Uuid;

use super::model::{BrandDto, BrandLocationsDto, BrandSearchDto, LocationsDto};
use super::repo;
use super::upstream::Upstream;
use super::wikidata;
use crate::accounts::repo as accounts_repo;
use crate::error::ApiError;

/// Searches shorter than this are too vague for a useful Wikidata lookup.
const MIN_QUERY_CHARS: usize = 2;
const MAX_QUERY_CHARS: usize = 64;
const LOCAL_RESULTS: u32 = 5;

pub async fn search(
    pool: &SqlitePool,
    upstream: &Upstream,
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
    let linked = repo::search(pool, query, LOCAL_RESULTS).await?;
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

/// Replaces the brand's shops with a fresh Overpass result.
pub async fn refresh_locations(
    pool: &SqlitePool,
    upstream: &Upstream,
    brand_id: Uuid,
    qid: &str,
    now: i64,
) -> Result<usize, ApiError> {
    let locations = match upstream.store_locations(qid).await {
        Ok(locations) => locations,
        Err(error) => {
            tracing::warn!(%error, qid, "Overpass query failed");
            repo::mark_failed(pool, brand_id, now).await?;
            return Err(ApiError::Unavailable);
        }
    };
    let mut tx = pool.begin().await?;
    repo::delete_locations(&mut *tx, brand_id).await?;
    for location in &locations {
        repo::insert_location(&mut *tx, brand_id, location).await?;
    }
    repo::mark_synced(&mut *tx, brand_id, now).await?;
    tx.commit().await?;
    tracing::info!(qid, count = locations.len(), "store locations refreshed");
    Ok(locations.len())
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
