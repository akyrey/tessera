import {
  deleteCard,
  deleteImage,
  fetchChanges,
  imageUrl,
  recordUse,
  saveCard,
  setFavorite,
  setSharing,
  uploadImage,
} from '../api/cards'
import { fetchLocations } from '../api/brands'
import { isApiError } from '../api/client'
import type { CardDto } from '../api/generated/CardDto'
import { applyOp, nowSeconds } from './applyOp'
import { type CardOp, type OutboxEntry, dataGeneration, db, readMeta, writeMeta } from './db'

const BASE_RETRY_MS = 2_000
/** Short cap: a missed `online` event must not leave changes waiting for long. */
const MAX_RETRY_MS = 60_000
const POLL_INTERVAL_MS = 60_000
/** Before the first pull; the server's cursor is never negative. */
const NO_CURSOR = -1
/** A change the server keeps failing on (5xx) is given up after this many tries. */
export const MAX_SERVER_ATTEMPTS = 8

/** Thrown when local data was wiped while a sync was running. */
class StaleSyncError extends Error {}

export interface SyncStatus {
  /** `false` when the browser reports no network or the last attempt could not reach the server. */
  readonly online: boolean
  readonly syncing: boolean
  readonly lastSyncedAt: number | null
}

interface Session {
  readonly userId: string
  readonly onUnauthorized: () => void
}

let session: Session | null = null
let status: SyncStatus = { online: true, syncing: false, lastSyncedAt: null }
let running: Promise<void> | null = null
let rerun = false
let retryTimer: ReturnType<typeof setTimeout> | undefined
let failures = 0
/** Outbox entry being sent right now: it must not absorb newer edits. */
let inFlightSeq: number | undefined
let stopActive: (() => void) | null = null
const listeners = new Set<() => void>()

export function getSyncStatus(): SyncStatus {
  return status
}

export function subscribeSyncStatus(listener: () => void): () => void {
  listeners.add(listener)
  return () => listeners.delete(listener)
}

function setStatus(patch: Partial<SyncStatus>): void {
  status = { ...status, ...patch }
  listeners.forEach((listener) => listener())
}

/** Starts background sync for a signed-in user; returns the stop function. */
export function startSync(userId: string, onUnauthorized: () => void): () => void {
  stopSync()
  session = { userId, onUnauthorized }
  const onOnline = () => {
    failures = 0
    void requestSync()
  }
  const onOffline = () => setStatus({ online: false })
  const onVisible = () => {
    if (document.visibilityState === 'visible') void requestSync()
  }
  window.addEventListener('online', onOnline)
  window.addEventListener('offline', onOffline)
  document.addEventListener('visibilitychange', onVisible)
  const poll = setInterval(onVisible, POLL_INTERVAL_MS)
  setStatus({ online: navigator.onLine })
  void requestSync()

  const stop = () => {
    session = null
    clearTimeout(retryTimer)
    clearInterval(poll)
    window.removeEventListener('online', onOnline)
    window.removeEventListener('offline', onOffline)
    document.removeEventListener('visibilitychange', onVisible)
  }
  stopActive = stop
  return stop
}

/** Stops background sync (e.g. before signing out); safe to call twice. */
export function stopSync(): void {
  stopActive?.()
  stopActive = null
}

/** Applies a change on this device right away and queues it for the server. */
export async function enqueue(cardId: string, op: CardOp): Promise<void> {
  const me = await readMeta('me')
  const userId = session?.userId ?? me?.user.id ?? ''
  const userName = me?.user.displayName ?? ''
  await db.transaction('rw', db.cards, db.outbox, async () => {
    const next = applyOp(await db.cards.get(cardId), op, { cardId, userId, userName, now: nowSeconds() })
    if (next) await db.cards.put(next)
    else await db.cards.delete(cardId)

    const pending = db.outbox.where('cardId').equals(cardId)
    if (op.type === 'delete') await pending.delete()
    if (op.type === 'save') {
      const queuedSave = await pending.filter((entry) => entry.op.type === 'save').first()
      // Only the latest content matters; keep the original position in the queue.
      // (Not into the entry being sent: it is deleted once the server answers.)
      if (queuedSave?.seq !== undefined && queuedSave.seq !== inFlightSeq) {
        return void (await db.outbox.update(queuedSave.seq, { op }))
      }
    }
    await db.outbox.add({ cardId, op, attempts: 0 })
  })
  void requestSync()
}

/** Sends queued changes, then pulls. Concurrent calls share one run. */
export function requestSync(): Promise<void> {
  if (!session) return Promise.resolve()
  if (running) {
    rerun = true
    return running
  }
  running = (async () => {
    do {
      rerun = false
      await runOnce()
    } while (rerun && session)
  })().finally(() => {
    running = null
  })
  return running
}

async function runOnce(): Promise<void> {
  clearTimeout(retryTimer)
  setStatus({ syncing: true })
  const startedIn = dataGeneration()
  try {
    await flushOutbox()
    await pull(startedIn)
    failures = 0
    setStatus({ online: true, syncing: false, lastSyncedAt: Date.now() })
  } catch (error) {
    setStatus({ syncing: false })
    handleFailure(error)
  }
}

function handleFailure(error: unknown): void {
  if (error instanceof StaleSyncError) return
  if (isApiError(error, 401)) {
    session?.onUnauthorized()
    return
  }
  if (isApiError(error) && error.code === 'network_error') setStatus({ online: false })
  failures += 1
  retryTimer = setTimeout(() => void requestSync(), retryDelay(failures))
}

/** Exponential backoff: 2 s, 4 s, 8 s… capped at 1 minute. */
export function retryDelay(failures: number): number {
  return Math.min(MAX_RETRY_MS, BASE_RETRY_MS * 2 ** Math.max(0, failures - 1))
}

/** Resolves once no sync is running (without starting one). */
export function whenIdle(): Promise<void> {
  return running ?? Promise.resolve()
}

// ---------- push ----------

async function flushOutbox(): Promise<void> {
  for (;;) {
    const entry = await db.outbox.orderBy('seq').first()
    if (entry?.seq === undefined) return
    inFlightSeq = entry.seq
    try {
      await send(entry)
    } catch (error) {
      if (isRefused(error)) {
        await reportRefusal(entry, error)
      } else if (isApiError(error) && error.status >= 500 && entry.attempts + 1 >= MAX_SERVER_ATTEMPTS) {
        await reportProblem(entry, 'the server kept failing')
      } else {
        await db.outbox.update(entry.seq, { attempts: entry.attempts + 1 })
        throw error
      }
      // The server may now differ from what this device shows: pull a full snapshot.
      await db.meta.delete('cursor')
    } finally {
      inFlightSeq = undefined
    }
    await db.outbox.delete(entry.seq)
  }
}

function send({ cardId, op }: OutboxEntry): Promise<unknown> {
  switch (op.type) {
    case 'save':
      return saveCard(cardId, op.input)
    case 'delete':
      return deleteCard(cardId)
    case 'favorite':
      return setFavorite(cardId, op.isFavorite)
    case 'share':
      return setSharing(cardId, op.shared)
    case 'use':
      return recordUse(cardId)
    case 'photo':
      return op.photo
        ? uploadImage(cardId, op.kind, new Blob([op.photo.bytes], { type: op.photo.mime }))
        : deleteImage(cardId, op.kind)
  }
}

/** The server answered and will never accept this change: drop it instead of retrying. */
function isRefused(error: unknown): boolean {
  if (!isApiError(error)) return false
  return error.status >= 400 && error.status < 500 && ![401, 408, 429].includes(error.status)
}

async function reportRefusal(entry: OutboxEntry, error: unknown): Promise<void> {
  // A card deleted elsewhere: changes to it no longer matter.
  if (isApiError(error, 404) || isApiError(error, 410)) return
  await reportProblem(entry, isApiError(error) ? error.message : 'unknown error')
}

async function reportProblem(entry: OutboxEntry, reason: string): Promise<void> {
  const name = (await db.cards.get(entry.cardId))?.storeName ?? 'a card'
  const problems = (await readMeta('problems')) ?? []
  await writeMeta('problems', [...problems, `Changes to ${name} could not be saved: ${reason}`])
}

// ---------- pull ----------

async function pull(startedIn: number): Promise<void> {
  // A cursor belongs to the user it was fetched for.
  const me = await readMeta('me')
  const cursorIsMine = me !== undefined && me.user.id === session?.userId
  const since = cursorIsMine ? ((await readMeta('cursor')) ?? NO_CURSOR) : NO_CURSOR
  const { cursor, cards } = await fetchChanges(since)
  await db.transaction('rw', db.cards, db.outbox, db.meta, async () => {
    if (dataGeneration() !== startedIn || !session) throw new StaleSyncError()
    if (cards) await replaceCards(cards)
    await writeMeta('cursor', cursor)
  })
  if (cards) warmPhotoCache(cards)
  await pullLocations(startedIn)
}

/** Shop locations of linked brands, for "nearby" (only when they changed). */
async function pullLocations(startedIn: number): Promise<void> {
  const known = await readMeta('locationsVersion')
  const { version, radiusM, brands } = await fetchLocations(known)
  await db.transaction('rw', db.locations, db.meta, async () => {
    if (dataGeneration() !== startedIn || !session) throw new StaleSyncError()
    if (brands) {
      await db.locations.clear()
      await db.locations.bulkPut(brands)
    }
    await writeMeta('locationsVersion', version)
    await writeMeta('radiusM', radiusM)
  })
}

/** Adopts the server's snapshot, keeping local versions of cards with unsent changes. */
async function replaceCards(serverCards: readonly CardDto[]): Promise<void> {
  const pending = new Set((await db.outbox.toArray()).map((entry) => entry.cardId))
  const keep = (await db.cards.toArray()).filter((card) => pending.has(card.id))
  await db.cards.clear()
  await db.cards.bulkPut([...serverCards.filter((card) => !pending.has(card.id)), ...keep])
}

/** Fetches photos through the service worker so they are cached before going offline. */
function warmPhotoCache(cards: readonly CardDto[]): void {
  if (!navigator.serviceWorker?.controller) return
  const ids = cards.flatMap((card) => [card.frontImageId, card.backImageId]).filter((id) => id !== null)
  for (const id of ids) void fetch(imageUrl(id), { credentials: 'same-origin' }).catch(() => undefined)
}

/** Test hook: forget timers and state between tests. */
export function resetSyncForTests(): void {
  clearTimeout(retryTimer)
  stopActive = null
  inFlightSeq = undefined
  session = null
  running = null
  rerun = false
  failures = 0
  status = { online: true, syncing: false, lastSyncedAt: null }
}
