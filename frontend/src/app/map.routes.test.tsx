import { screen } from '@testing-library/react'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import * as accounts from '../api/accounts'
import * as brandsApi from '../api/brands'
import * as cards from '../api/cards'
import { card, owner } from '../test/fixtures'
import { renderRoute } from '../test/renderRoute'

vi.mock('../api/accounts')
vi.mock('../api/brands')
vi.mock('../api/cards')
// Leaflet needs a real layout engine; the map itself is covered by Playwright.
vi.mock('../features/map/ShopMap', () => ({
  default: ({ markers }: { markers: readonly unknown[] }) => <p>{markers.length} shops on the map</p>,
}))

beforeEach(() => {
  vi.mocked(accounts.fetchMe).mockResolvedValue(owner)
  vi.mocked(brandsApi.fetchLocations).mockResolvedValue({
    version: 'v1',
    radiusM: 200,
    brands: [
      {
        brandId: 'b1',
        points: [
          [45, 9],
          [46, 10],
        ],
      },
    ],
  })
})

describe('map', () => {
  it('is linked from the card list and shows every known shop', async () => {
    vi.mocked(cards.fetchChanges).mockResolvedValue({
      cursor: 1,
      cards: [card({ brandId: 'b1' }), card({ id: 'c2', places: [{ label: 'Forno', lat: 1, lon: 2 }] })],
    })

    Object.defineProperty(navigator, 'geolocation', {
      value: { getCurrentPosition: vi.fn() },
      configurable: true,
    })
    const { user } = renderRoute('/')
    await user.click(await screen.findByRole('link', { name: 'Map' }))

    expect(await screen.findByText('3 shops on the map')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Where am I?' })).toBeInTheDocument()
    Reflect.deleteProperty(navigator, 'geolocation')
  })

  it('explains an empty map', async () => {
    vi.mocked(cards.fetchChanges).mockResolvedValue({ cursor: 1, cards: [card()] })

    renderRoute('/map')

    expect(await screen.findByText(/No shops yet/)).toBeInTheDocument()
  })
})
