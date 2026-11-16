import { describe, expect, it, vi } from 'vitest'
import { fetchLocations, linkBrand, searchBrands } from './brands'

function respond(body: unknown) {
  const fetchMock = vi
    .fn<typeof fetch>()
    .mockResolvedValue(
      new Response(JSON.stringify(body), { headers: { 'Content-Type': 'application/json' } }),
    )
  vi.stubGlobal('fetch', fetchMock)
  return fetchMock
}

const brand = { id: 'b1', wikidataId: 'Q1', name: 'Coop', locationCount: 3, lastSyncedAt: null }

describe('brands api', () => {
  it('searches with an encoded query', async () => {
    const fetchMock = respond({ linked: [brand], candidates: [], remoteUnavailable: false })
    await expect(searchBrands('coop & co')).resolves.toMatchObject({ linked: [brand] })
    expect(fetchMock.mock.calls[0]?.[0]).toBe('/api/brands/search?q=coop%20%26%20co')
  })

  it('links a brand by Wikidata id', async () => {
    const fetchMock = respond(brand)
    await expect(linkBrand('Q1')).resolves.toEqual(brand)
    expect(fetchMock.mock.calls[0]?.[1]).toMatchObject({ method: 'POST', body: '{"wikidataId":"Q1"}' })
  })

  it('fetches locations with and without a known version', async () => {
    const fetchMock = respond({ version: 'v1', radiusM: 200, brands: [{ brandId: 'b1', points: [[45, 9]] }] })
    await expect(fetchLocations(undefined)).resolves.toMatchObject({ version: 'v1' })
    expect(fetchMock.mock.calls[0]?.[0]).toBe('/api/locations')
    respond({ version: 'v1', radiusM: 200, brands: null })
    await fetchLocations('v1')
    expect(vi.mocked(fetch).mock.calls[0]?.[0]).toBe('/api/locations?version=v1')
  })
})
