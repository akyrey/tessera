import Dexie, { type EntityTable } from 'dexie'
import type { CardDto } from '../api/generated/CardDto'
import type { CardInput } from '../api/generated/CardInput'
import type { ImageKind } from '../api/generated/ImageKind'
import type { Me } from '../api/generated/Me'

/** A card as stored on the device: the server's view plus edits not yet synced. */
export interface LocalCard extends CardDto {
  /** Photos changed on this device: a `data:` preview, or `null` when removed. */
  readonly localPhotos?: Partial<Record<ImageKind, string | null>>
}

/** A change made on this device, waiting to be sent to the server. */
export type CardOp =
  | {
      readonly type: 'save'
      readonly input: CardInput
      /** Display name of `input.brandId`, shown until the server confirms it. */
      readonly brandName?: string | null
    }
  | { readonly type: 'delete' }
  | { readonly type: 'favorite'; readonly isFavorite: boolean }
  | { readonly type: 'share'; readonly shared: boolean }
  | { readonly type: 'use'; readonly at: number }
  | {
      readonly type: 'photo'
      readonly kind: ImageKind
      /** Raw bytes, not a Blob: Safari cannot store Blobs in IndexedDB in private mode. */
      readonly photo: StoredPhoto | null
      readonly preview: string | null
    }

export interface StoredPhoto {
  readonly bytes: ArrayBuffer
  readonly mime: string
}

export interface OutboxEntry {
  readonly seq?: number
  readonly cardId: string
  readonly op: CardOp
  /** Failed attempts so far (network or server errors). */
  readonly attempts: number
}

/** Shops of one brand, as `[lat, lon]` pairs (from OpenStreetMap, via the server). */
export interface BrandLocations {
  readonly brandId: string
  readonly points: readonly (readonly [number, number])[]
}

export interface MetaEntries {
  /** Last `cursor` from `/api/changes`; absent until the first successful pull. */
  readonly cursor: number
  /** Signed-in user, for opening the app without network. */
  readonly me: Me
  /** Changes the server refused, shown to the user once. */
  readonly problems: readonly string[]
  /** Version of the stored shop locations, from `/api/locations`. */
  readonly locationsVersion: string
  /** "Nearby" distance configured on the server, in metres. */
  readonly radiusM: number
}

type MetaRow = {
  [K in keyof MetaEntries]: { readonly key: K; readonly value: MetaEntries[K] }
}[keyof MetaEntries]

export class TesseraDb extends Dexie {
  cards!: EntityTable<LocalCard, 'id'>
  outbox!: EntityTable<OutboxEntry, 'seq'>
  meta!: EntityTable<MetaRow, 'key'>
  locations!: EntityTable<BrandLocations, 'brandId'>

  constructor() {
    super('tessera')
    this.version(1).stores({
      cards: 'id',
      outbox: '++seq, cardId',
      meta: 'key',
    })
    this.version(2).stores({ locations: 'brandId' })
  }
}

export const db = new TesseraDb()

export async function readMeta<K extends keyof MetaEntries>(key: K): Promise<MetaEntries[K] | undefined> {
  const row = await db.meta.get(key)
  return row?.value as MetaEntries[K] | undefined
}

export async function writeMeta<K extends keyof MetaEntries>(key: K, value: MetaEntries[K]): Promise<void> {
  await db.meta.put({ key, value } as MetaRow)
}

let generation = 0

/** Bumped by every wipe: sync work started before it must not write afterwards. */
export function dataGeneration(): number {
  return generation
}

/** Removes every trace of the signed-in user from this device. */
export async function clearLocalData(): Promise<void> {
  generation += 1
  await db.transaction('rw', [db.cards, db.outbox, db.meta, db.locations], async () => {
    await Promise.all([db.cards.clear(), db.outbox.clear(), db.meta.clear(), db.locations.clear()])
  })
  if ('caches' in globalThis) await caches.delete(PHOTO_CACHE).catch(() => false)
  // Workbox's expiration bookkeeping lists the cached photo URLs.
  globalThis.indexedDB?.deleteDatabase('workbox-expiration')
}

/** Service worker cache for `/api/images/*` (see vite.config.ts). */
export const PHOTO_CACHE = 'card-photos'
