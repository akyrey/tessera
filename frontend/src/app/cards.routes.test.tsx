import { screen, waitFor, within } from '@testing-library/react'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import * as accounts from '../api/accounts'
import * as brandsApi from '../api/brands'
import * as cards from '../api/cards'
import { ApiError } from '../api/client'
import type { CardDto } from '../api/generated/CardDto'
import * as photos from '../features/cards/photos'
import * as zxing from '../features/scan/zxing'
import { db, writeMeta } from '../offline/db'
import { whenIdle } from '../offline/syncEngine'
import { card, member, owner } from '../test/fixtures'
import { renderRoute } from '../test/renderRoute'

vi.mock('../api/accounts')
vi.mock('../api/brands')
vi.mock('../api/cards')
vi.mock('../features/cards/photos', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../features/cards/photos')>()),
  preparePhoto: vi.fn(),
}))
vi.mock('../features/scan/zxing', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../features/scan/zxing')>()),
  scanImage: vi.fn(),
}))

const esselunga = card({ id: 'c-esse', storeName: 'Esselunga', memberName: 'Alice' })
const coop = card({
  id: 'c-coop',
  storeName: 'Coop',
  barcodeFormat: 'qr',
  cardNumber: 'COOP-1',
  isFavorite: true,
})
const decathlon = card({ id: 'c-deca', storeName: 'Decathlon', lastUsedAt: 1_800_000_500 })
const offline = () => new ApiError(0, 'network_error', 'Cannot reach the server')

/** What the server returns on the next pulls. */
function serverHas(...serverCards: CardDto[]) {
  vi.mocked(cards.fetchChanges).mockResolvedValue({ cursor: 1, cards: serverCards })
}

beforeEach(() => {
  vi.mocked(brandsApi.fetchLocations).mockResolvedValue({ version: 'v0', radiusM: 200, brands: [] })
  vi.mocked(accounts.fetchMe).mockResolvedValue(owner)
  vi.mocked(cards.imageUrl).mockImplementation((id) => `/api/images/${id}`)
  vi.mocked(cards.saveCard).mockImplementation(async (id) => card({ id }))
  serverHas(esselunga, coop, decathlon)
})

const section = (name: string) => within(screen.getByRole('region', { name }))

describe('card list', () => {
  it('groups favourites, recently used and the rest', async () => {
    renderRoute('/')

    await screen.findByRole('region', { name: 'Favourites' })
    expect(section('Favourites').getByRole('link', { name: /Coop/ })).toBeInTheDocument()
    expect(section('Recently used').getByRole('link', { name: /Decathlon/ })).toBeInTheDocument()
    expect(section('All other cards').getByRole('link', { name: /Esselunga/ })).toHaveAttribute(
      'href',
      '/cards/c-esse',
    )
  })

  it('filters by search text', async () => {
    const { user } = renderRoute('/')

    await user.type(await screen.findByRole('searchbox', { name: 'Search cards' }), 'alice')

    const results = section('Search results')
    expect(results.getAllByRole('link')).toHaveLength(1)
    expect(results.getByRole('link', { name: /Esselunga/ })).toBeInTheDocument()

    await user.clear(screen.getByRole('searchbox'))
    await user.type(screen.getByRole('searchbox'), 'ikea')
    expect(screen.getByText('No cards match “ikea”.')).toBeInTheDocument()
  })

  it('invites to add the first card', async () => {
    serverHas()

    renderRoute('/')

    expect(await screen.findByRole('heading', { name: 'No cards yet' })).toBeInTheDocument()
    expect(screen.getByRole('link', { name: 'Add card' })).toHaveAttribute('href', '/cards/new')
  })

  it('shows the cards saved on the device when offline', async () => {
    await writeMeta('me', owner)
    await db.cards.bulkPut([esselunga, coop])
    await writeMeta('cursor', 1)
    vi.mocked(cards.fetchChanges).mockRejectedValue(offline())

    renderRoute('/')

    expect(await screen.findByText(/Offline: showing the cards saved on this device/)).toBeInTheDocument()
    expect(await screen.findByRole('link', { name: /Esselunga/ })).toBeInTheDocument()
  })

  it('keeps waiting for the first sync instead of claiming the wallet is empty', async () => {
    vi.mocked(cards.fetchChanges).mockRejectedValue(offline())

    renderRoute('/')

    expect(await screen.findByText('Loading cards…')).toBeInTheDocument()
    expect(screen.queryByRole('heading', { name: 'No cards yet' })).not.toBeInTheDocument()
  })

  it('reports changes the server refused until dismissed', async () => {
    await writeMeta('me', owner)
    await writeMeta('problems', ['Changes to Coop could not be saved: store name is too long'])

    const { user } = renderRoute('/')

    expect(await screen.findByRole('alert')).toHaveTextContent('store name is too long')
    await user.click(screen.getByRole('button', { name: 'Dismiss' }))
    await waitFor(() => expect(screen.queryByRole('alert')).not.toBeInTheDocument())
  })
})

describe('card view', () => {
  it('shows the barcode and number and counts the use', async () => {
    renderRoute('/cards/c-esse')

    expect(await screen.findByRole('heading', { name: 'Esselunga' })).toBeInTheDocument()
    expect(await screen.findByRole('img', { name: /EAN-13/ })).toBeInTheDocument()
    expect(screen.getByText('4006381333931')).toBeInTheDocument()
    await waitFor(() => expect(cards.recordUse).toHaveBeenCalledWith('c-esse'))
    expect(cards.recordUse).toHaveBeenCalledTimes(1)
  })

  it('shows photos and notes', async () => {
    serverHas({ ...esselunga, frontImageId: 'img-f', notes: 'Tessera fedeltà' })

    renderRoute('/cards/c-esse')

    expect(await screen.findByRole('img', { name: 'Front of the card' })).toHaveAttribute(
      'src',
      '/api/images/img-f',
    )
    expect(screen.getByText('Tessera fedeltà')).toBeInTheDocument()
  })

  it('toggles the favourite at once and syncs it', async () => {
    const { user } = renderRoute('/cards/c-esse')
    const star = await screen.findByRole('button', { name: 'Favourite' })
    expect(star).toHaveAttribute('aria-pressed', 'false')
    await user.click(star)

    await waitFor(() => expect(star).toHaveAttribute('aria-pressed', 'true'))
    await waitFor(() => expect(cards.setFavorite).toHaveBeenCalledWith('c-esse', true))
  })

  it('reports cards that do not exist', async () => {
    renderRoute('/cards/missing')

    expect(await screen.findByText('This card does not exist or was deleted.')).toBeInTheDocument()
  })
})

describe('new card', () => {
  it('validates before saving', async () => {
    const { user } = renderRoute('/cards/new')

    await user.type(await screen.findByLabelText('Card number'), '4006381333932')
    expect(screen.getByText(/last digit does not match/)).toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: 'Save card' }))

    expect(screen.getByText('Store name is required')).toBeInTheDocument()
    expect(screen.getByLabelText('Card number')).toHaveAttribute('aria-invalid', 'true')
    expect(await db.outbox.count()).toBe(0)
  })

  it('saves a card with a photo, opens it and syncs it', async () => {
    const photo = new Blob(['x'], { type: 'image/webp' })
    vi.mocked(photos.preparePhoto).mockResolvedValue(photo)
    vi.mocked(cards.uploadImage).mockResolvedValue({ id: 'img-new' })
    // The server does not know the card until the outbox has sent it.
    vi.mocked(cards.fetchChanges).mockResolvedValue({ cursor: 1, cards: null })

    const { user, router } = renderRoute('/cards/new')
    await user.type(await screen.findByLabelText('Store name'), '  Bennet ')
    await user.selectOptions(screen.getByLabelText('Barcode type'), 'code39')
    await user.type(screen.getByLabelText('Card number'), 'ab-12')
    await user.click(screen.getByRole('radio', { name: '#1d4ed8' }))
    await user.upload(
      screen.getByLabelText('Front photo'),
      new File(['raw'], 'front.jpg', { type: 'image/jpeg' }),
    )
    expect(await screen.findByRole('img', { name: 'Front' })).toHaveAttribute(
      'src',
      expect.stringMatching(/^data:image\/webp;base64,/),
    )
    await user.click(screen.getByRole('button', { name: 'Save card' }))

    await waitFor(() => expect(router.state.location.pathname).toMatch(/^\/cards\/[0-9a-f-]{36}$/))
    expect(await screen.findByRole('heading', { name: 'Bennet' })).toBeInTheDocument()
    expect(screen.getByRole('img', { name: 'Front of the card' })).toHaveAttribute(
      'src',
      expect.stringMatching(/^data:/),
    )
    await waitFor(() => expect(cards.uploadImage).toHaveBeenCalled())
    const [id, input] = vi.mocked(cards.saveCard).mock.calls[0] ?? []
    expect(input).toEqual({
      storeName: 'Bennet',
      cardNumber: 'AB-12',
      barcodeFormat: 'code39',
      memberName: undefined,
      color: '#1d4ed8',
      notes: undefined,
      brandId: undefined,
      places: [],
    })
    expect(router.state.location.pathname).toBe(`/cards/${id}`)
  })

  it('saves offline and keeps the change for later', async () => {
    vi.mocked(cards.saveCard).mockRejectedValue(offline())

    const { user } = renderRoute('/cards/new')
    await user.type(await screen.findByLabelText('Store name'), 'Coop Lombardia')
    await user.selectOptions(screen.getByLabelText('Barcode type'), 'none')
    await user.type(screen.getByLabelText('Card number'), '123')
    await user.click(screen.getByRole('button', { name: 'Save card' }))

    expect(await screen.findByRole('heading', { name: 'Coop Lombardia' })).toBeInTheDocument()
    await whenIdle()
    expect(await db.outbox.count()).toBeGreaterThan(0)
  })

  it('fills the form from a scanned photo', async () => {
    vi.mocked(zxing.scanImage).mockResolvedValue({
      status: 'found',
      format: 'qr',
      text: 'https://x.example/1',
    })

    const { user } = renderRoute('/cards/new')
    await user.upload(
      await screen.findByLabelText('Barcode photo'),
      new File(['img'], 'b.png', { type: 'image/png' }),
    )

    await waitFor(() => expect(screen.getByLabelText('Card number')).toHaveValue('https://x.example/1'))
    expect(screen.getByLabelText('Barcode type')).toHaveValue('qr')
  })

  it('explains scans that find nothing usable', async () => {
    vi.mocked(zxing.scanImage).mockResolvedValueOnce({ status: 'unsupported', formatName: 'MaxiCode' })
    vi.mocked(zxing.scanImage).mockResolvedValueOnce({ status: 'none' })

    const { user } = renderRoute('/cards/new')
    const input = await screen.findByLabelText('Barcode photo')
    await user.upload(input, new File(['img'], 'a.png', { type: 'image/png' }))
    expect(await screen.findByRole('alert')).toHaveTextContent('MaxiCode')
    await user.upload(input, new File(['img'], 'b.png', { type: 'image/png' }))
    expect(await screen.findByRole('alert')).toHaveTextContent('No barcode found')
  })

  it('explains photos that cannot be used', async () => {
    vi.mocked(photos.preparePhoto).mockRejectedValue(new photos.PhotoError('This image cannot be read.'))

    const { user } = renderRoute('/cards/new')
    await user.upload(
      await screen.findByLabelText('Back photo'),
      new File(['raw'], 'b.heic', { type: 'image/heic' }),
    )

    expect(await screen.findByRole('alert')).toHaveTextContent('This image cannot be read.')
  })
})

describe('edit card', () => {
  it('is prefilled and can remove an existing photo', async () => {
    serverHas({ ...esselunga, backImageId: 'img-b' })

    const { user, router } = renderRoute('/cards/c-esse/edit')
    expect(await screen.findByLabelText('Store name')).toHaveValue('Esselunga')
    expect(screen.getByLabelText('Name on the card (optional)')).toHaveValue('Alice')
    expect(screen.getByRole('img', { name: 'Back' })).toHaveAttribute('src', '/api/images/img-b')
    await user.click(screen.getByRole('button', { name: 'Remove' }))
    expect(screen.queryByRole('img', { name: 'Back' })).not.toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: 'Save card' }))

    await waitFor(() => expect(router.state.location.pathname).toBe('/cards/c-esse'))
    await waitFor(() => expect(cards.deleteImage).toHaveBeenCalledWith('c-esse', 'back'))
  })

  it('discards a newly picked photo without deleting the stored one', async () => {
    serverHas({ ...esselunga, frontImageId: 'img-f' })
    vi.mocked(photos.preparePhoto).mockResolvedValue(new Blob(['x'], { type: 'image/webp' }))

    const { user } = renderRoute('/cards/c-esse/edit')
    await user.upload(
      await screen.findByLabelText('Front photo'),
      new File(['raw'], 'f.jpg', { type: 'image/jpeg' }),
    )
    await waitFor(() =>
      expect(screen.getByRole('img', { name: 'Front' }).getAttribute('src')).toMatch(/^data:/),
    )
    await user.click(screen.getByRole('button', { name: 'Remove' }))

    expect(screen.getByRole('img', { name: 'Front' })).toHaveAttribute('src', '/api/images/img-f')
  })

  it('deletes after a second confirmation tap', async () => {
    const { user, router } = renderRoute('/cards/c-esse/edit')
    await user.click(await screen.findByRole('button', { name: 'Delete card' }))
    expect(cards.deleteCard).not.toHaveBeenCalled()
    await user.click(screen.getByRole('button', { name: 'Tap again to delete for good' }))

    await waitFor(() => expect(router.state.location.pathname).toBe('/'))
    await waitFor(() => expect(cards.deleteCard).toHaveBeenCalledWith('c-esse'))
  })

  it('does not offer deletion to someone who does not own the card', async () => {
    vi.mocked(accounts.fetchMe).mockResolvedValue(member)

    renderRoute('/cards/c-esse/edit')

    expect(await screen.findByLabelText('Store name')).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'Delete card' })).not.toBeInTheDocument()
  })
})

describe('sharing', () => {
  it('lets the owner share a card with the household', async () => {
    vi.mocked(cards.setSharing).mockImplementation(async () => {
      serverHas({ ...esselunga, shared: true })
      return { ...esselunga, shared: true }
    })

    const { user, router } = renderRoute('/cards/c-esse/edit')
    const toggle = await screen.findByRole('switch', { name: /Share with Home/ })
    expect(toggle).not.toBeChecked()
    await user.click(toggle)
    await user.click(screen.getByRole('button', { name: 'Save card' }))

    await waitFor(() => expect(router.state.location.pathname).toBe('/cards/c-esse'))
    expect(await screen.findByText('Shared with Home')).toBeInTheDocument()
    await waitFor(() => expect(cards.setSharing).toHaveBeenCalledWith('c-esse', true))
  })

  it('does not resend sharing when it did not change', async () => {
    const { user, router } = renderRoute('/cards/c-esse/edit')
    await user.click(await screen.findByRole('button', { name: 'Save card' }))

    await waitFor(() => expect(router.state.location.pathname).toBe('/cards/c-esse'))
    await whenIdle()
    expect(cards.setSharing).not.toHaveBeenCalled()
  })

  it("shows who shared a household member's card, without sharing controls", async () => {
    vi.mocked(accounts.fetchMe).mockResolvedValue(member)
    serverHas({ ...esselunga, shared: true })

    const { user } = renderRoute('/')
    expect(await screen.findByRole('link', { name: /Esselunga.*Alice/ })).toBeInTheDocument()
    await user.click(screen.getByRole('link', { name: /Esselunga/ }))
    expect(await screen.findByText('Shared by Alice')).toBeInTheDocument()

    await user.click(screen.getByRole('link', { name: 'Edit' }))
    await screen.findByLabelText('Store name')
    expect(screen.queryByRole('switch')).not.toBeInTheDocument()
  })

  it('marks own shared cards in the list', async () => {
    serverHas({ ...esselunga, shared: true })

    renderRoute('/')

    expect(await screen.findByRole('link', { name: /Esselunga.*Shared/ })).toBeInTheDocument()
  })
})
