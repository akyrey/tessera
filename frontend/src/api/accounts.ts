import { z } from 'zod'
import { getJson, isApiError, postJson, sendRequest } from './client'
import type { CeremonyStart } from './generated/CeremonyStart'
import type { InviteCreated } from './generated/InviteCreated'
import type { InvitePreview } from './generated/InvitePreview'
import type { Me } from './generated/Me'
import type { PasskeyDto } from './generated/PasskeyDto'
import type { RegisterStartRequest } from './generated/RegisterStartRequest'
import {
  ceremonyStartSchema,
  inviteCreatedSchema,
  invitePreviewSchema,
  meSchema,
  passkeySchema,
} from './schemas'

/** The signed-in user, or `null` when there is no valid session. */
export async function fetchMe(signal?: AbortSignal): Promise<Me | null> {
  try {
    return await getJson('/api/me', meSchema, signal)
  } catch (error) {
    if (isApiError(error, 401)) return null
    throw error
  }
}

export function previewInvite(token: string): Promise<InvitePreview> {
  return postJson('/api/auth/invite', invitePreviewSchema, { token })
}

export function startRegistration(request: RegisterStartRequest): Promise<CeremonyStart> {
  return postJson('/api/auth/register/start', ceremonyStartSchema, request)
}

export function finishRegistration(
  ceremonyId: string,
  credential: unknown,
  passkeyName: string,
): Promise<Me> {
  return postJson('/api/auth/register/finish', meSchema, { ceremonyId, credential, passkeyName })
}

export function startLogin(): Promise<CeremonyStart> {
  return postJson('/api/auth/login/start', ceremonyStartSchema)
}

export function finishLogin(ceremonyId: string, credential: unknown): Promise<Me> {
  return postJson('/api/auth/login/finish', meSchema, { ceremonyId, credential })
}

export function logout(): Promise<void> {
  return sendRequest('POST', '/api/auth/logout')
}

/** Ends every session of the current user, including this one. */
export function signOutEverywhere(): Promise<void> {
  return sendRequest('DELETE', '/api/me/sessions')
}

export function listPasskeys(signal?: AbortSignal): Promise<PasskeyDto[]> {
  return getJson('/api/me/passkeys', z.array(passkeySchema), signal)
}

export function startAddPasskey(): Promise<CeremonyStart> {
  return postJson('/api/me/passkeys/start', ceremonyStartSchema)
}

export function finishAddPasskey(
  ceremonyId: string,
  credential: unknown,
  passkeyName: string,
): Promise<PasskeyDto> {
  return postJson('/api/me/passkeys/finish', passkeySchema, { ceremonyId, credential, passkeyName })
}

export function deletePasskey(id: string): Promise<void> {
  return sendRequest('DELETE', `/api/me/passkeys/${encodeURIComponent(id)}`)
}

export function createHouseholdInvite(): Promise<InviteCreated> {
  return postJson('/api/household/invites', inviteCreatedSchema)
}
