import type { BrandLocations, LocalCard } from '../../offline/db'

export interface ShopMarker {
  readonly lat: number
  readonly lon: number
  readonly cardId: string
  readonly storeName: string
  /** `place` = marked by hand; `brand` = from OpenStreetMap. */
  readonly source: 'brand' | 'place'
}

/** One marker per known shop of each card (a brand's shops appear once per card using it). */
export function shopMarkers(cards: readonly LocalCard[], locations: readonly BrandLocations[]): ShopMarker[] {
  const byBrand = new Map(locations.map((entry) => [entry.brandId, entry.points]))
  return cards.flatMap((card) => [
    ...(card.brandId ? (byBrand.get(card.brandId) ?? []) : []).map(([lat, lon]) => ({
      lat,
      lon,
      cardId: card.id,
      storeName: card.storeName,
      source: 'brand' as const,
    })),
    ...card.places.map((place) => ({
      lat: place.lat,
      lon: place.lon,
      cardId: card.id,
      storeName: card.storeName,
      source: 'place' as const,
    })),
  ])
}
