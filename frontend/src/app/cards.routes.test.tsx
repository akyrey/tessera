import { screen, waitFor, within } from '@testing-library/react'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import * as accounts from '../api/accounts'
import * as cards from '../api/cards'
import { ApiError } from '../api/client'
import * as photos from '../features/cards/photos'
import { card, member, owner } from '../test/fixtures'
import { renderRoute } from '../test/renderRoute'

vi.mock('../api/accounts')
vi.mock('../api/cards')
vi.mock('../features/cards/photos', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../features/cards/photos')>()),
  preparePhoto: vi.fn(),
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

beforeEach(() => {
  vi.mocked(accounts.fetchMe).mockResolvedValue(owner)
  vi.mocked(cards.listCards).mockResolvedValue([esselunga, coop, decathlon])
  vi.mocked(cards.recordUse).mockResolvedValue()
  vi.mocked(cards.imageUrl).mockImplementation((id) => `/api/images/${id}`)
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
    vi.mocked(cards.listCards).mockResolvedValue([])

    renderRoute('/')

    expect(await screen.findByRole('heading', { name: 'No cards yet' })).toBeInTheDocument()
    expect(screen.getByRole('link', { name: 'Add card' })).toHaveAttribute('href', '/cards/new')
  })

  it('offers a retry when loading fails', async () => {
    vi.mocked(cards.listCards).mockRejectedValueOnce(new ApiError(0, 'network_error', 'offline'))

    const { user } = renderRoute('/')

    expect(await screen.findByRole('alert')).toHaveTextContent('Cannot reach the server')
    await user.click(screen.getByRole('button', { name: 'Try again' }))
    expect(await screen.findByRole('region', { name: 'Favourites' })).toBeInTheDocument()
  })
})

describe('card view', () => {
  it('shows the barcode and number and counts the use', async () => {
    renderRoute('/cards/c-esse')

    expect(await screen.findByRole('heading', { name: 'Esselunga' })).toBeInTheDocument()
    expect(await screen.findByRole('img', { name: /EAN-13/ })).toBeInTheDocument()
    expect(screen.getByText('4006381333931')).toBeInTheDocument()
    await waitFor(() => expect(cards.recordUse).toHaveBeenCalledTimes(1))
    expect(cards.recordUse).toHaveBeenCalledWith('c-esse')
  })

  it('shows photos and notes', async () => {
    vi.mocked(cards.listCards).mockResolvedValue([
      { ...esselunga, frontImageId: 'img-f', notes: 'Tessera fedeltà' },
    ])

    renderRoute('/cards/c-esse')

    expect(await screen.findByRole('img', { name: 'Front of the card' })).toHaveAttribute(
      'src',
      '/api/images/img-f',
    )
    expect(screen.getByText('Tessera fedeltà')).toBeInTheDocument()
  })

  it('toggles the favourite', async () => {
    vi.mocked(cards.setFavorite).mockResolvedValue({ ...esselunga, isFavorite: true })

    const { user } = renderRoute('/cards/c-esse')
    const star = await screen.findByRole('button', { name: 'Favourite' })
    expect(star).toHaveAttribute('aria-pressed', 'false')
    await user.click(star)

    expect(cards.setFavorite).toHaveBeenCalledWith('c-esse', true)
    await waitFor(() => expect(star).toHaveAttribute('aria-pressed', 'true'))
  })

  it('reports cards that do not exist', async () => {
    renderRoute('/cards/missing')

    expect(await screen.findByText('This card does not exist or was deleted.')).toBeInTheDocument()
  })

  it('still shows the card when counting the use fails', async () => {
    vi.mocked(cards.recordUse).mockRejectedValue(new ApiError(0, 'network_error', 'offline'))

    renderRoute('/cards/c-esse')

    expect(await screen.findByRole('heading', { name: 'Esselunga' })).toBeInTheDocument()
    expect(screen.queryByRole('alert')).not.toBeInTheDocument()
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
    expect(cards.saveCard).not.toHaveBeenCalled()
  })

  it('saves a card with a photo and opens it', async () => {
    const photo = new Blob(['x'], { type: 'image/webp' })
    vi.mocked(photos.preparePhoto).mockResolvedValue(photo)
    const stored = [esselunga]
    vi.mocked(cards.listCards).mockImplementation(async () => [...stored])
    vi.mocked(cards.saveCard).mockImplementation(async (id, input) => {
      const saved = card({
        id,
        storeName: input.storeName,
        cardNumber: input.cardNumber,
        barcodeFormat: input.barcodeFormat,
      })
      stored.push(saved)
      return saved
    })
    vi.mocked(cards.uploadImage).mockResolvedValue({ id: 'img-new' })

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
    const [id, input] = vi.mocked(cards.saveCard).mock.calls[0] ?? []
    expect(input).toEqual({
      storeName: 'Bennet',
      cardNumber: 'AB-12',
      barcodeFormat: 'code39',
      memberName: undefined,
      color: '#1d4ed8',
      notes: undefined,
    })
    expect(cards.uploadImage).toHaveBeenCalledWith(id, 'front', photo)
    expect(await screen.findByRole('heading', { name: 'Bennet' })).toBeInTheDocument()
  })

  it('shows server errors and keeps the form', async () => {
    vi.mocked(cards.saveCard).mockRejectedValue(
      new ApiError(400, 'invalid_request', 'store name is too long'),
    )

    const { user } = renderRoute('/cards/new')
    await user.type(await screen.findByLabelText('Store name'), 'Coop')
    await user.selectOptions(screen.getByLabelText('Barcode type'), 'none')
    await user.type(screen.getByLabelText('Card number'), '123')
    await user.click(screen.getByRole('button', { name: 'Save card' }))

    expect(await screen.findByRole('alert')).toHaveTextContent('Store name is too long')
    expect(screen.getByLabelText('Store name')).toHaveValue('Coop')
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
  it('discards a newly picked photo without deleting the stored one', async () => {
    vi.mocked(cards.listCards).mockResolvedValue([{ ...esselunga, frontImageId: 'img-f' }])
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

  it('refreshes the list when saving fails part-way', async () => {
    vi.mocked(cards.saveCard).mockRejectedValue(new ApiError(0, 'network_error', 'offline'))

    const { user } = renderRoute('/cards/c-esse/edit')
    await user.click(await screen.findByRole('button', { name: 'Save card' }))

    expect(await screen.findByRole('alert')).toHaveTextContent('Cannot reach the server')
    await waitFor(() => expect(cards.listCards).toHaveBeenCalledTimes(2))
  })

  it('is prefilled and can remove an existing photo', async () => {
    vi.mocked(cards.listCards).mockResolvedValue([{ ...esselunga, backImageId: 'img-b' }])
    vi.mocked(cards.saveCard).mockResolvedValue({ ...esselunga, backImageId: 'img-b' })
    vi.mocked(cards.deleteImage).mockResolvedValue()

    const { user, router } = renderRoute('/cards/c-esse/edit')
    expect(await screen.findByLabelText('Store name')).toHaveValue('Esselunga')
    expect(screen.getByLabelText('Name on the card (optional)')).toHaveValue('Alice')
    expect(screen.getByRole('img', { name: 'Back' })).toHaveAttribute('src', '/api/images/img-b')
    await user.click(screen.getByRole('button', { name: 'Remove' }))
    expect(screen.queryByRole('img', { name: 'Back' })).not.toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: 'Save card' }))

    await waitFor(() => expect(router.state.location.pathname).toBe('/cards/c-esse'))
    expect(cards.deleteImage).toHaveBeenCalledWith('c-esse', 'back')
  })

  it('deletes after a second confirmation tap', async () => {
    vi.mocked(cards.deleteCard).mockResolvedValue()

    const { user, router } = renderRoute('/cards/c-esse/edit')
    await user.click(await screen.findByRole('button', { name: 'Delete card' }))
    expect(cards.deleteCard).not.toHaveBeenCalled()
    await user.click(screen.getByRole('button', { name: 'Tap again to delete for good' }))

    await waitFor(() => expect(router.state.location.pathname).toBe('/'))
    expect(cards.deleteCard).toHaveBeenCalledWith('c-esse')
  })

  it('does not offer deletion to someone who does not own the card', async () => {
    vi.mocked(accounts.fetchMe).mockResolvedValue(member)

    renderRoute('/cards/c-esse/edit')

    expect(await screen.findByLabelText('Store name')).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'Delete card' })).not.toBeInTheDocument()
  })
})
