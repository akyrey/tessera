import type { ImageKind } from '../../api/generated/ImageKind'
import type { LocalCard } from '../../offline/db'
import { photoSrc } from '../cards/photoSrc'
import { BACKUP_FORMAT, BACKUP_VERSION, type Backup, type BackupCard } from './backupFormat'

const PHOTO_TYPES = ['image/jpeg', 'image/png', 'image/webp']

/** Builds the backup of `cards`, embedding photos (from the network or the offline cache). */
export async function buildBackup(cards: readonly LocalCard[], now: Date = new Date()): Promise<Backup> {
  return {
    format: BACKUP_FORMAT,
    version: BACKUP_VERSION,
    exportedAt: now.toISOString(),
    cards: await Promise.all(cards.map(toBackupCard)),
  }
}

async function toBackupCard(card: LocalCard): Promise<BackupCard> {
  const [front, back] = await Promise.all([photoData(card, 'front'), photoData(card, 'back')])
  return {
    storeName: card.storeName,
    cardNumber: card.cardNumber,
    barcodeFormat: card.barcodeFormat,
    memberName: card.memberName,
    color: card.color,
    notes: card.notes,
    expiresOn: card.expiresOn,
    isFavorite: card.isFavorite,
    brandName: card.brandName,
    places: card.places,
    photos: { ...(front && { front }), ...(back && { back }) },
  }
}

/** The photo as a `data:` URL, or `undefined` if there is none or it cannot be read now. */
async function photoData(card: LocalCard, kind: ImageKind): Promise<string | undefined> {
  const src = photoSrc(card, kind)
  if (!src) return undefined
  if (src.startsWith('data:')) return src
  try {
    const response = await fetch(src, { credentials: 'same-origin' })
    const type = response.headers.get('content-type') ?? ''
    // Only the types a backup may contain (and the server accepts on import).
    if (!response.ok || !PHOTO_TYPES.includes(type)) return undefined
    return await blobToDataUrl(new Blob([await response.arrayBuffer()], { type }))
  } catch {
    return undefined
  }
}

function blobToDataUrl(blob: Blob): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader()
    reader.onload = () => resolve(String(reader.result))
    reader.onerror = () => reject(reader.error)
    reader.readAsDataURL(blob)
  })
}

/** Offers the backup as a file download. */
export function downloadBackup(backup: Backup): void {
  const blob = new Blob([JSON.stringify(backup, null, 2)], { type: 'application/json' })
  const url = URL.createObjectURL(blob)
  const link = document.createElement('a')
  link.href = url
  link.download = `tessera-cards-${backup.exportedAt.slice(0, 10)}.json`
  link.click()
  setTimeout(() => URL.revokeObjectURL(url), 1_000)
}
