import { z } from 'zod'
import type { CeremonyStart } from './generated/CeremonyStart'
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
