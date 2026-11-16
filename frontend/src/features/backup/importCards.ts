import type { BarcodeFormat } from '../../api/generated/BarcodeFormat'
import type { ImageKind } from '../../api/generated/ImageKind'
import type { LocalCard } from '../../offline/db'
import { enqueue } from '../../offline/syncEngine'
import { checkCardNumber } from '../cards/barcodeFormats'
import { uuidv7 } from '../cards/uuid'
import { type Backup, type BackupCard, MAX_BACKUP_BYTES, backupSchema } from './backupFormat'

export interface ImportResult {
  readonly imported: number
  readonly skipped: number
}

export class BackupError extends Error {
  constructor(message: string) {
    super(message)
    this.name = 'BackupError'
  }
}

/** Reads and validates a backup file; never trusts its content. */
export async function readBackup(file: Blob): Promise<Backup> {
  if (file.size > MAX_BACKUP_BYTES) throw new BackupError('This file is too large to be a Tessera backup.')
  let raw: unknown
  try {
    raw = JSON.parse(await file.text())
  } catch {
    throw new BackupError('This file is not a Tessera backup (it is not JSON).')
  }
  const parsed = backupSchema.safeParse(raw)
  if (!parsed.success) throw new BackupError('This file is not a valid Tessera backup.')
  return parsed.data
}

const identity = (format: BarcodeFormat, number: string) => `${format}:${number}`

/**
 * Adds the backup's cards as new cards of the current user, skipping cards
 * already in the wallet (same format and number) or with an invalid number.
 */
export async function importBackup(backup: Backup, existing: readonly LocalCard[]): Promise<ImportResult> {
  const known = new Set(existing.map((card) => identity(card.barcodeFormat, card.cardNumber)))
  let imported = 0
  for (const card of backup.cards) {
    const check = checkCardNumber(card.barcodeFormat, card.cardNumber)
    if (!check.ok || known.has(identity(card.barcodeFormat, check.value))) continue
    known.add(identity(card.barcodeFormat, check.value))
    await importCard(card, check.value)
    imported += 1
  }
  return { imported, skipped: backup.cards.length - imported }
}

async function importCard(card: BackupCard, cardNumber: string): Promise<void> {
  const id = uuidv7()
  await enqueue(id, {
    type: 'save',
    input: {
      storeName: card.storeName,
      cardNumber,
      barcodeFormat: card.barcodeFormat,
      memberName: card.memberName ?? undefined,
      color: card.color ?? undefined,
      notes: card.notes ?? undefined,
      expiresOn: card.expiresOn ?? undefined,
    },
  })
  if (card.places.length > 0) await enqueue(id, { type: 'places', places: card.places })
  if (card.isFavorite) await enqueue(id, { type: 'favorite', isFavorite: true })
  for (const kind of ['front', 'back'] as const satisfies readonly ImageKind[]) {
    const dataUrl = card.photos[kind]
    if (dataUrl) await enqueue(id, { type: 'photo', kind, photo: dataUrlToPhoto(dataUrl), preview: dataUrl })
  }
}

function dataUrlToPhoto(dataUrl: string): { bytes: ArrayBuffer; mime: string } {
  const [header = '', base64 = ''] = dataUrl.split(',')
  const mime = header.slice('data:'.length, header.indexOf(';'))
  const binary = atob(base64)
  const bytes = new Uint8Array(binary.length)
  for (let index = 0; index < binary.length; index += 1) bytes[index] = binary.charCodeAt(index)
  return { bytes: bytes.buffer, mime }
}
