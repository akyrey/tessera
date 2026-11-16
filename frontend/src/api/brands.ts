import { getJson, postJson } from './client'
import type { BrandDto } from './generated/BrandDto'
import type { BrandSearchDto } from './generated/BrandSearchDto'
import type { LocationsDto } from './generated/LocationsDto'
import { brandSchema, brandSearchSchema, locationsSchema } from './schemas'

export function searchBrands(query: string, signal?: AbortSignal): Promise<BrandSearchDto> {
  return getJson(`/api/brands/search?q=${encodeURIComponent(query)}`, brandSearchSchema, signal)
}

/** Links a Wikidata item as a brand (the server fetches its name and shops). */
export function linkBrand(wikidataId: string): Promise<BrandDto> {
  return postJson('/api/brands', brandSchema, { wikidataId })
}

/** Shops of the brands on visible cards; `brands: null` when `version` is current. */
export function fetchLocations(version: string | undefined, signal?: AbortSignal): Promise<LocationsDto> {
  const query = version ? `?version=${encodeURIComponent(version)}` : ''
  return getJson(`/api/locations${query}`, locationsSchema, signal)
}
