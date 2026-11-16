import { z } from 'zod'
import { barcodeFormatSchema, placeSchema } from '../../api/schemas'

export const BACKUP_FORMAT = 'tessera-backup'
export const BACKUP_VERSION = 1
/** Photos are ≤ 512 KB each; this leaves room for a few hundred cards. */
export const MAX_BACKUP_BYTES = 50 * 1024 * 1024

const photoSchema = z
  .string()
  .regex(/^data:image\/(jpeg|png|webp);base64,[A-Za-z0-9+/=]+$/)
  .max(1024 * 1024)

export const backupCardSchema = z.object({
  storeName: z.string().min(1).max(64),
  cardNumber: z.string().min(1).max(512),
  barcodeFormat: barcodeFormatSchema,
  memberName: z.string().max(64).nullable(),
  color: z.string().nullable(),
  notes: z.string().max(1000).nullable(),
  expiresOn: z.string().nullable(),
  isFavorite: z.boolean(),
  /** For reference only: brands are linked again from the card form. */
  brandName: z.string().nullable(),
  places: z.array(placeSchema).max(20),
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
