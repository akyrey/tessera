import { describe, expect, it, vi } from 'vitest'
import { card } from '../test/fixtures'
import {
  deleteCard,
  deleteImage,
  fetchChanges,
  imageUrl,
  listCards,
  recordUse,
  saveCard,
  setFavorite,
  setSharing,
  uploadImage,
} from './cards'

function respond(body: unknown, status = 200) {
  const fetchMock = vi.fn<typeof fetch>().mockResolvedValue(
    new Response(body === null ? null : JSON.stringify(body), {
      status,
      headers: { 'Content-Type': 'application/json' },
    }),
  )
  vi.stubGlobal('fetch', fetchMock)
  return fetchMock
}

function lastCall(fetchMock: ReturnType<typeof respond>) {
  const [path, init] = fetchMock.mock.calls.at(-1) ?? []
  return { path, method: init?.method, body: init?.body }
}

describe('cards api', () => {
  it('lists and validates cards', async () => {
    respond([card()])
    await expect(listCards()).resolves.toEqual([card()])
  })

  it('rejects malformed card lists', async () => {
    respond([{ ...card(), barcodeFormat: 'maxicode' }])
    await expect(listCards()).rejects.toMatchObject({ code: 'invalid_response' })
  })

  it('saves a card with PUT on its id', async () => {
    const fetchMock = respond(card())
    const input = { storeName: 'Esselunga', cardNumber: '4006381333931', barcodeFormat: 'ean13' } as const

    await saveCard('c/1', input)

    expect(lastCall(fetchMock)).toEqual({
      path: '/api/cards/c%2F1',
      method: 'PUT',
      body: JSON.stringify(input),
    })
  })

  it('deletes, favourites and records use', async () => {
    const fetchMock = respond(null, 204)
    await deleteCard('c1')
    expect(lastCall(fetchMock)).toMatchObject({ path: '/api/cards/c1', method: 'DELETE' })
    await recordUse('c1')
    expect(lastCall(fetchMock)).toMatchObject({ path: '/api/cards/c1/use', method: 'POST' })
    await deleteImage('c1', 'back')
    expect(lastCall(fetchMock)).toMatchObject({ path: '/api/cards/c1/images/back', method: 'DELETE' })

    respond(card({ isFavorite: true }))
    await expect(setFavorite('c1', true)).resolves.toMatchObject({ isFavorite: true })

    const sharing = respond(card({ shared: true }))
    await expect(setSharing('c1', true)).resolves.toMatchObject({ shared: true })
    expect(lastCall(sharing)).toEqual({
      path: '/api/cards/c1/sharing',
      method: 'PUT',
      body: '{"shared":true}',
    })
  })

  it('uploads photos as raw bodies', async () => {
    const fetchMock = respond({ id: 'img-1' })
    const photo = new Blob(['x'], { type: 'image/jpeg' })

    await expect(uploadImage('c1', 'front', photo)).resolves.toEqual({ id: 'img-1' })

    expect(lastCall(fetchMock)).toEqual({ path: '/api/cards/c1/images/front', method: 'PUT', body: photo })
  })

  it('pulls changes after a cursor', async () => {
    const fetchMock = respond({ cursor: 7, cards: null })
    await expect(fetchChanges(3)).resolves.toEqual({ cursor: 7, cards: null })
    expect(lastCall(fetchMock).path).toBe('/api/changes?since=3')
  })

  it('builds image urls', () => {
    expect(imageUrl('img-1')).toBe('/api/images/img-1')
  })
})
