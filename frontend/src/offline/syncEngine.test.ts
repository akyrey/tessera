import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import * as api from '../api/cards'
import { ApiError } from '../api/client'
import { card } from '../test/fixtures'
import { db, readMeta } from './db'
import { enqueue, getSyncStatus, requestSync, retryDelay, startSync, whenIdle } from './syncEngine'

vi.mock('../api/cards')

const input = { storeName: 'Coop', cardNumber: '123', barcodeFormat: 'none' } as const
const offline = () => new ApiError(0, 'network_error', 'Cannot reach the server')
let stop: () => void = () => undefined

beforeEach(() => {
  vi.mocked(api.fetchChanges).mockResolvedValue({ cursor: 1, cards: [] })
  vi.mocked(api.saveCard).mockImplementation(async (id) => card({ id }))
})

afterEach(() => stop())

async function start(onUnauthorized = vi.fn()) {
  stop = startSync('u-alice', onUnauthorized)
  await whenIdle()
  return onUnauthorized
}

describe('pull', () => {
  it('stores the snapshot and the cursor', async () => {
    vi.mocked(api.fetchChanges).mockResolvedValue({
      cursor: 9,
      cards: [card({ id: 'a' }), card({ id: 'b' })],
    })

    await start()

    expect((await db.cards.toArray()).map((c) => c.id).sort()).toEqual(['a', 'b'])
    expect(await readMeta('cursor')).toBe(9)
    expect(api.fetchChanges).toHaveBeenCalledWith(-1)
    expect(getSyncStatus()).toMatchObject({ online: true, syncing: false })
  })

  it('keeps local cards when nothing changed, and drops cards gone from the server', async () => {
    vi.mocked(api.fetchChanges).mockResolvedValueOnce({
      cursor: 2,
      cards: [card({ id: 'a' }), card({ id: 'b' })],
    })
    await start()

    vi.mocked(api.fetchChanges).mockResolvedValueOnce({ cursor: 2, cards: null })
    await requestSync()
    expect(await db.cards.count()).toBe(2)
    expect(api.fetchChanges).toHaveBeenLastCalledWith(2)

    vi.mocked(api.fetchChanges).mockResolvedValueOnce({ cursor: 3, cards: [card({ id: 'b' })] })
    await requestSync()
    expect((await db.cards.toArray()).map((c) => c.id)).toEqual(['b'])
  })
})

describe('outbox', () => {
  it('applies changes locally at once and sends them in order', async () => {
    await start()
    vi.mocked(api.fetchChanges).mockResolvedValue({
      cursor: 5,
      cards: [card({ id: 'c1', storeName: 'Coop' })],
    })

    await enqueue('c1', { type: 'save', input })
    expect((await db.cards.get('c1'))?.storeName).toBe('Coop')
    await enqueue('c1', { type: 'favorite', isFavorite: true })
    await requestSync()

    expect(api.saveCard).toHaveBeenCalledWith('c1', input)
    expect(api.setFavorite).toHaveBeenCalledWith('c1', true)
    expect(vi.mocked(api.saveCard).mock.invocationCallOrder[0]).toBeLessThan(
      vi.mocked(api.setFavorite).mock.invocationCallOrder[0] ?? 0,
    )
    expect(await db.outbox.count()).toBe(0)
  })

  it('keeps changes while offline and shows local data over the server snapshot', async () => {
    await start()
    vi.mocked(api.saveCard).mockRejectedValue(offline())

    await enqueue('c1', { type: 'save', input })
    await requestSync()

    expect(getSyncStatus().online).toBe(false)
    expect(await db.outbox.count()).toBe(1)
    expect((await db.outbox.toArray())[0]?.attempts).toBeGreaterThan(0)
    expect((await db.cards.get('c1'))?.storeName).toBe('Coop')
  })

  it('coalesces repeated saves of the same card', async () => {
    vi.mocked(api.saveCard).mockRejectedValue(offline())
    await start()
    await enqueue('c1', { type: 'save', input })
    await enqueue('c1', { type: 'save', input: { ...input, storeName: 'Coop 2' } })

    const queued = await db.outbox.toArray()
    expect(queued).toHaveLength(1)
    expect(queued[0]?.op).toEqual({ type: 'save', input: { ...input, storeName: 'Coop 2' } })
  })

  it('a deletion replaces pending changes and survives the next pull', async () => {
    vi.mocked(api.fetchChanges).mockResolvedValue({ cursor: 1, cards: [card({ id: 'c1' })] })
    await start()
    vi.mocked(api.deleteCard).mockRejectedValue(offline())

    await enqueue('c1', { type: 'favorite', isFavorite: true })
    await enqueue('c1', { type: 'delete' })
    await requestSync()

    expect((await db.outbox.toArray()).map((e) => e.op.type)).toEqual(['delete'])
    expect(await db.cards.get('c1')).toBeUndefined()
  })

  it('drops changes the server refuses and reports them', async () => {
    await start()
    vi.mocked(api.saveCard).mockRejectedValue(new ApiError(400, 'invalid_request', 'store name is too long'))

    await enqueue('c1', { type: 'save', input })
    await requestSync()

    expect(await db.outbox.count()).toBe(0)
    expect(await readMeta('problems')).toEqual(['Changes to Coop could not be saved: store name is too long'])
  })

  it('quietly drops changes to cards deleted elsewhere', async () => {
    await start()
    vi.mocked(api.recordUse).mockRejectedValue(new ApiError(404, 'not_found', 'resource not found'))

    await enqueue('c1', { type: 'save', input })
    await enqueue('c1', { type: 'use', at: 1 })
    await requestSync()

    expect(await db.outbox.count()).toBe(0)
    expect(await readMeta('problems')).toBeUndefined()
  })

  it('uploads and removes photos', async () => {
    await start()
    const photo = { bytes: new Uint8Array([1, 2]).buffer, mime: 'image/webp' }
    await enqueue('c1', { type: 'save', input })
    await enqueue('c1', { type: 'photo', kind: 'front', photo, preview: 'data:x' })
    await enqueue('c1', { type: 'photo', kind: 'back', photo: null, preview: null })
    await requestSync()

    const sent = vi.mocked(api.uploadImage).mock.calls[0]?.[2]
    expect(sent?.type).toBe('image/webp')
    expect(sent?.size).toBe(2)
    expect(api.deleteImage).toHaveBeenCalledWith('c1', 'back')
  })
})

describe('session', () => {
  it('reports an expired session', async () => {
    vi.mocked(api.fetchChanges).mockRejectedValue(new ApiError(401, 'unauthorized', 'sign in required'))

    const onUnauthorized = await start()

    expect(onUnauthorized).toHaveBeenCalled()
  })

  it('backs off exponentially up to one minute', () => {
    expect([1, 2, 3, 4].map(retryDelay)).toEqual([2_000, 4_000, 8_000, 16_000])
    expect(retryDelay(50)).toBe(60_000)
  })

  it('retries at once when the network comes back', async () => {
    vi.mocked(api.fetchChanges).mockRejectedValueOnce(offline())
    await start()
    expect(getSyncStatus().online).toBe(false)

    window.dispatchEvent(new Event('online'))
    await whenIdle()

    expect(api.fetchChanges).toHaveBeenCalledTimes(2)
    expect(getSyncStatus().online).toBe(true)
  })

  it('does nothing when no one is signed in', async () => {
    await requestSync()
    expect(api.fetchChanges).not.toHaveBeenCalled()
  })
})
