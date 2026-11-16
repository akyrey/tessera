import { z } from 'zod'
import type { BarcodeFormat } from './generated/BarcodeFormat'
import type { CardDto } from './generated/CardDto'
import type { CeremonyStart } from './generated/CeremonyStart'
import type { ChangesDto } from './generated/ChangesDto'
import type { ImageCreated } from './generated/ImageCreated'
import type { HouseholdDto } from './generated/HouseholdDto'
import type { InviteCreated } from './generated/InviteCreated'
import type { InvitePreview } from './generated/InvitePreview'
import type { Me } from './generated/Me'
import type { MemberDto } from './generated/MemberDto'
import type { PasskeyDto } from './generated/PasskeyDto'
import type { UserDto } from './generated/UserDto'

// Runtime validation of server responses. `satisfies` keeps each schema in
// lockstep with the types generated from the Rust DTOs.

const role = z.enum(['owner', 'member'])

export const memberSchema = z.object({
  userId: z.string(),
  displayName: z.string(),
  role,
  joinedAt: z.number(),
}) satisfies z.ZodType<MemberDto>

export const householdSchema = z.object({
  id: z.string(),
  name: z.string(),
  role,
  members: z.array(memberSchema),
}) satisfies z.ZodType<HouseholdDto>

export const userSchema = z.object({
  id: z.string(),
  displayName: z.string(),
  isAdmin: z.boolean(),
}) satisfies z.ZodType<UserDto>

export const meSchema = z.object({ user: userSchema, household: householdSchema }) satisfies z.ZodType<Me>

export const passkeySchema = z.object({
  id: z.string(),
  name: z.string(),
  createdAt: z.number(),
  lastUsedAt: z.number().nullable(),
}) satisfies z.ZodType<PasskeyDto>

export const invitePreviewSchema = z.object({
  kind: z.enum(['bootstrap', 'household', 'recovery']),
  householdName: z.string().nullable(),
  displayName: z.string().nullable(),
}) satisfies z.ZodType<InvitePreview>

export const inviteCreatedSchema = z.object({
  url: z.string(),
  expiresAt: z.number(),
}) satisfies z.ZodType<InviteCreated>

export const ceremonyStartSchema = z.object({
  ceremonyId: z.string(),
  options: z.unknown(),
}) satisfies z.ZodType<CeremonyStart>

export const BARCODE_FORMATS = [
  'ean13',
  'ean8',
  'upca',
  'code128',
  'code39',
  'itf',
  'codabar',
  'qr',
  'pdf417',
  'aztec',
  'datamatrix',
  'none',
] as const satisfies readonly BarcodeFormat[]

export const barcodeFormatSchema = z.enum(BARCODE_FORMATS) satisfies z.ZodType<BarcodeFormat>

export const cardSchema = z.object({
  id: z.string(),
  ownerId: z.string(),
  storeName: z.string(),
  cardNumber: z.string(),
  barcodeFormat: barcodeFormatSchema,
  memberName: z.string().nullable(),
  color: z.string().nullable(),
  notes: z.string().nullable(),
  isFavorite: z.boolean(),
  useCount: z.number(),
  lastUsedAt: z.number().nullable(),
  frontImageId: z.string().nullable(),
  backImageId: z.string().nullable(),
  createdAt: z.number(),
  updatedAt: z.number(),
}) satisfies z.ZodType<CardDto>

export const imageCreatedSchema = z.object({ id: z.string() }) satisfies z.ZodType<ImageCreated>

export const changesSchema = z.object({
  cursor: z.number(),
  cards: z.array(cardSchema).nullable(),
}) satisfies z.ZodType<ChangesDto>
