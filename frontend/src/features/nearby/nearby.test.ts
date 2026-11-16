import { describe, expect, it } from 'vitest'
import { card } from '../../test/fixtures'
import { distanceMeters, findNearby, formatDistance } from './nearby'

const duomo = { lat: 45.4642, lon: 9.19 }

describe('distanceMeters', () => {
  it('matches known distances', () => {
    // Milano Duomo → Castello Sforzesco is about 1.15 km.
    expect(distanceMeters(duomo, { lat: 45.4705, lon: 9.1793 })).toBeGreaterThan(1050)
    expect(distanceMeters(duomo, { lat: 45.4705, lon: 9.1793 })).toBeLessThan(1250)
    expect(distanceMeters(duomo, duomo)).toBe(0)
    // Milano → Roma, about 477 km.
    expect(Math.round(distanceMeters(duomo, { lat: 41.9028, lon: 12.4964 }) / 1000)).toBeCloseTo(477, -1)
  })
})

describe('findNearby', () => {
  const points = new Map([['b-esse', [[45.465, 9.19] as const, [41.9, 12.5] as const]]])

  it('finds cards by brand shops and by hand-marked places, closest first', () => {
    const esse = card({ id: 'esse', brandId: 'b-esse' })
    const bakery = card({ id: 'bakery', places: [{ label: 'Forno', lat: 45.4643, lon: 9.1901 }] })
    const far = card({ id: 'far', places: [{ label: 'Roma', lat: 41.9, lon: 12.5 }] })
    const unlinked = card({ id: 'unlinked' })

    const result = findNearby([esse, bakery, far, unlinked], points, duomo, 200)

    expect(result.map((entry) => entry.card.id)).toEqual(['bakery', 'esse'])
    expect(result[1]?.distanceM).toBeGreaterThan(80)
  })

  it('ignores brands without known shops', () => {
    expect(findNearby([card({ brandId: 'unknown' })], points, duomo, 5000)).toEqual([])
  })
})

describe('formatDistance', () => {
  it('rounds to tens of metres, then kilometres', () => {
    expect(formatDistance(3)).toBe('10 m')
    expect(formatDistance(84)).toBe('80 m')
    expect(formatDistance(1234)).toBe('1.2 km')
  })
})
