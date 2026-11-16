import { z } from 'zod'
import { barcodeFormatSchema } from '../../api/schemas'

export const BACKUP_FORMAT = 'tessera-backup'
export const BACKUP_VERSION = 1
/** Photos are ≤ 512 KB each; this leaves room for a few hundred cards. */
export const MAX_BACKUP_BYTES = 50 * 1024 * 1024

/** The server's photo limit, in bytes. */
const MAX_PHOTO_BYTES = 512 * 1024
/** `data:image/jpeg;base64,` plus the base64 length of the largest photo the server accepts. */
const MAX_PHOTO_DATA_URL = 'data:image/jpeg;base64,'.length + 4 * Math.ceil(MAX_PHOTO_BYTES / 3)

const photoSchema = z
  .string()
  .regex(/^data:image\/(jpeg|png|webp);base64,[A-Za-z0-9+/=]+$/)
  .max(MAX_PHOTO_DATA_URL)

/** The same limits the server checks, so a bad file fails here and not later in the outbox. */
const backupPlaceSchema = z.object({
  label: z.string().trim().min(1).max(64),
  lat: z.number().min(-90).max(90),
  lon: z.number().min(-180).max(180),
})

export const backupCardSchema = z.object({
  storeName: z.string().min(1).max(64),
  cardNumber: z.string().min(1).max(512),
  barcodeFormat: barcodeFormatSchema,
  memberName: z.string().max(64).nullable(),
  color: z
    .string()
    .regex(/^#[0-9a-fA-F]{6}$/)
    .nullable(),
  notes: z.string().max(1000).nullable(),
  expiresOn: z
    .string()
    .regex(/^\d{4}-\d{2}-\d{2}$/)
    .nullable(),
  isFavorite: z.boolean(),
  /** For reference only: brands are linked again from the card form. */
  brandName: z.string().nullable(),
  places: z.array(backupPlaceSchema).max(20),
  photos: z.object({ front: photoSchema.optional(), back: photoSchema.optional() }),
})

export const backupSchema = z.object({
  format: z.literal(BACKUP_FORMAT),
  version: z.literal(BACKUP_VERSION),
  exportedAt: z.string(),
  cards: z.array(backupCardSchema).max(500),
})

export type BackupCard = z.infer<typeof backupCardSchema>
export type Backup = z.infer<typeof backupSchema>
