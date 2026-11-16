import { screen, waitFor, within } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import * as accounts from '../api/accounts'
import * as brandsApi from '../api/brands'
import * as cards from '../api/cards'
import { ApiError } from '../api/client'
import type { CardDto } from '../api/generated/CardDto'
import { card, owner } from '../test/fixtures'
import { renderRoute } from '../test/renderRoute'

vi.mock('../api/accounts')
vi.mock('../api/brands')
vi.mock('../api/cards')

const DUOMO = { latitude: 45.4642, longitude: 9.19 }
const esselunga = card({ id: 'c-esse', storeName: 'Esselunga', brandId: 'b-esse', brandName: 'Esselunga' })
const forno = card({
  id: 'c-forno',
  storeName: 'Forno',
  places: [{ label: 'Forno', lat: 45.4643, lon: 9.1901 }],
})
const coop = card({ id: 'c-coop', storeName: 'Coop' })

function serverHas(...serverCards: CardDto[]) {
  vi.mocked(cards.fetchChanges).mockResolvedValue({ cursor: 1, cards: serverCards })
}

/** Geolocation that answers with `coords`, and a permission already granted or not. */
function stubLocation(permission: PermissionState, result: GeolocationCoordinates | { code: number }) {
  const getCurrentPosition = vi.fn((success: PositionCallback, failure?: PositionErrorCallback | null) => {
    if ('latitude' in result) success({ coords: result } as GeolocationPosition)
    else failure?.({ code: result.code, PERMISSION_DENIED: 1 } as GeolocationPositionError)
  })
  Object.defineProperty(navigator, 'geolocation', { value: { getCurrentPosition }, configurable: true })
  Object.defineProperty(navigator, 'permissions', {
    value: { query: vi.fn().mockResolvedValue({ state: permission }) },
    configurable: true,
  })
  return getCurrentPosition
}

beforeEach(() => {
  vi.mocked(accounts.fetchMe).mockResolvedValue(owner)
  vi.mocked(cards.saveCard).mockImplementation(async (id) => card({ id }))
  vi.mocked(brandsApi.fetchLocations).mockResolvedValue({
    version: 'v1',
    radiusM: 200,
    brands: [{ brandId: 'b-esse', points: [[45.465, 9.19]] }],
  })
  serverHas(esselunga, forno, coop)
})

afterEach(() => {
  Reflect.deleteProperty(navigator, 'geolocation')
  Reflect.deleteProperty(navigator, 'permissions')
})

describe('nearby', () => {
  it('puts cards for nearby shops on top when location is already allowed', async () => {
    stubLocation('granted', DUOMO as GeolocationCoordinates)

    renderRoute('/')

    const nearby = within(await screen.findByRole('region', { name: 'Nearby' }))
    const links = nearby.getAllByRole('link')
    expect(links.map((link) => link.textContent)).toEqual(['Forno', 'Esselunga'])
    expect(nearby.getByText('10 m')).toBeInTheDocument()
  })

  it('asks before locating when permission was not given yet', async () => {
    const getCurrentPosition = stubLocation('prompt', DUOMO as GeolocationCoordinates)

    const { user } = renderRoute('/')
    const button = await screen.findByRole('button', { name: 'Show cards for shops near me' })
    expect(getCurrentPosition).not.toHaveBeenCalled()
    await user.click(button)

    expect(await screen.findByRole('region', { name: 'Nearby' })).toBeInTheDocument()
  })

  it('stays out of the way when location is denied', async () => {
    stubLocation('denied', { code: 1 })

    renderRoute('/')

    await screen.findByRole('region', { name: 'All cards' })
    await waitFor(() => expect(screen.queryByRole('button', { name: /near me/ })).not.toBeInTheDocument())
  })

  it('shows nothing extra when no shop is near', async () => {
    stubLocation('granted', { latitude: 41.9, longitude: 12.5 } as GeolocationCoordinates)

    renderRoute('/')

    await screen.findByRole('region', { name: 'All cards' })
    await waitFor(() => expect(screen.queryByRole('button', { name: /near me/ })).not.toBeInTheDocument())
    expect(screen.queryByRole('region', { name: 'Nearby' })).not.toBeInTheDocument()
  })
})

describe('brand picker', () => {
  it('links a Wikidata brand to a new card', async () => {
    vi.mocked(brandsApi.searchBrands).mockResolvedValue({
      linked: [],
      candidates: [{ wikidataId: 'Q1145962', name: 'Conad', description: 'catena di supermercati' }],
      remoteUnavailable: false,
    })
    vi.mocked(brandsApi.linkBrand).mockResolvedValue({
      id: 'b-conad',
      wikidataId: 'Q1145962',
      name: 'Conad',
      locationCount: 0,
      lastSyncedAt: null,
    })

    const { user } = renderRoute('/cards/new')
    await user.type(await screen.findByLabelText('Search store brand'), 'conad')
    await user.click(await screen.findByRole('button', { name: /Conad.*catena di supermercati/ }))
    expect(brandsApi.linkBrand).toHaveBeenCalledWith('Q1145962')
    await user.type(screen.getByLabelText('Store name'), 'Conad')
    await user.selectOptions(screen.getByLabelText('Barcode type'), 'none')
    await user.type(screen.getByLabelText('Card number'), '42')
    await user.click(screen.getByRole('button', { name: 'Save card' }))

    await waitFor(() => expect(cards.saveCard).toHaveBeenCalled())
    expect(vi.mocked(cards.saveCard).mock.calls[0]?.[1]).toMatchObject({ brandId: 'b-conad' })
  })

  it('shows known brands, explains failures, and can unlink', async () => {
    vi.mocked(brandsApi.searchBrands).mockResolvedValue({
      linked: [{ id: 'b-esse', wikidataId: 'Q1', name: 'Esselunga', locationCount: 160, lastSyncedAt: 1 }],
      candidates: [{ wikidataId: 'Q2', name: 'Esselunga Prima', description: null }],
      remoteUnavailable: true,
    })
    vi.mocked(brandsApi.linkBrand).mockRejectedValue(new ApiError(0, 'network_error', 'offline'))

    const { user } = renderRoute('/cards/c-esse/edit')
    expect(await screen.findByText('Esselunga', { selector: 'span' })).toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: 'Change' }))
    await user.type(screen.getByLabelText('Search store brand'), 'esse')
    expect(await screen.findByRole('button', { name: /160 shops/ })).toBeInTheDocument()
    expect(screen.getByText(/Wikidata is not reachable/)).toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: /Esselunga Prima/ }))
    expect(await screen.findByText('Cannot reach the server. Check your connection.')).toBeInTheDocument()

    await user.click(screen.getByRole('button', { name: /160 shops/ }))
    await user.click(screen.getByRole('button', { name: 'Remove' }))
    await user.click(screen.getByRole('button', { name: 'Save card' }))
    await waitFor(() => expect(cards.saveCard).toHaveBeenCalled())
    expect(vi.mocked(cards.saveCard).mock.calls[0]?.[1]).toMatchObject({ brandId: undefined })
  })
})

describe('marking shops by hand', () => {
  it('remembers the current location for the card and can remove it', async () => {
    stubLocation('prompt', DUOMO as GeolocationCoordinates)
    // A server that remembers saves, so the next pull agrees with the device.
    vi.mocked(cards.saveCard).mockImplementation(async (_id, input) => {
      const saved = { ...coop, places: input.places ?? [] }
      serverHas(esselunga, forno, saved)
      return saved
    })

    const { user } = renderRoute('/cards/c-coop')
    await user.click(await screen.findByRole('button', { name: /I am at this shop/ }))

    expect(await screen.findByText('Coop · 45.4642, 9.1900')).toBeInTheDocument()
    await waitFor(() =>
      expect(cards.saveCard).toHaveBeenCalledWith(
        'c-coop',
        expect.objectContaining({ places: [{ label: 'Coop', lat: 45.4642, lon: 9.19 }] }),
      ),
    )
    await user.click(screen.getByRole('button', { name: 'Remove location 1' }))
    await waitFor(() => expect(screen.queryByText(/Coop · /)).not.toBeInTheDocument())
  })

  it('explains a denied location', async () => {
    stubLocation('prompt', { code: 1 })

    const { user } = renderRoute('/cards/c-coop')
    await user.click(await screen.findByRole('button', { name: /I am at this shop/ }))

    expect(await screen.findByRole('alert')).toHaveTextContent('Location access was denied')
  })
})
