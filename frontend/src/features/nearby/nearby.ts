import type { LocalCard } from '../../offline/db'

export interface LatLon {
  readonly lat: number
  readonly lon: number
}

export interface NearbyCard {
  readonly card: LocalCard
  /** Distance to the closest known shop of this card, in metres. */
  readonly distanceM: number
}

const EARTH_RADIUS_M = 6_371_000
const toRadians = (degrees: number) => (degrees * Math.PI) / 180

/** Great-circle distance (haversine), accurate to well under a metre at shop scale. */
export function distanceMeters(a: LatLon, b: LatLon): number {
  const dLat = toRadians(b.lat - a.lat)
  const dLon = toRadians(b.lon - a.lon)
  const h =
    Math.sin(dLat / 2) ** 2 +
    Math.cos(toRadians(a.lat)) * Math.cos(toRadians(b.lat)) * Math.sin(dLon / 2) ** 2
  return 2 * EARTH_RADIUS_M * Math.asin(Math.min(1, Math.sqrt(h)))
}

/**
 * Cards with a shop (of their brand, or marked by hand) within `radiusM`,
 * closest first. Runs entirely on the device.
 */
export function findNearby(
  cards: readonly LocalCard[],
  brandPoints: ReadonlyMap<string, readonly (readonly [number, number])[]>,
  position: LatLon,
  radiusM: number,
): NearbyCard[] {
  return cards
    .map((card) => ({ card, distanceM: closestShop(card, brandPoints, position) }))
    .filter((entry) => entry.distanceM <= radiusM)
    .toSorted((a, b) => a.distanceM - b.distanceM)
}

function closestShop(
  card: LocalCard,
  brandPoints: ReadonlyMap<string, readonly (readonly [number, number])[]>,
  position: LatLon,
): number {
  const shops: LatLon[] = [
    ...(card.brandId ? (brandPoints.get(card.brandId) ?? []) : []).map(([lat, lon]) => ({ lat, lon })),
    ...card.places,
  ]
  return shops.reduce((closest, shop) => Math.min(closest, distanceMeters(position, shop)), Infinity)
}

/** "80 m", "1.2 km" */
export function formatDistance(meters: number): string {
  if (meters < 1000) return `${Math.max(10, Math.round(meters / 10) * 10)} m`
  return `${(meters / 1000).toFixed(1)} km`
}
