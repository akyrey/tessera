import { z } from 'zod'
import { getJson, putBlob, putJson, sendRequest } from './client'
import type { CardDto } from './generated/CardDto'
import type { CardInput } from './generated/CardInput'
import type { ChangesDto } from './generated/ChangesDto'
import type { ImageCreated } from './generated/ImageCreated'
import type { ImageKind } from './generated/ImageKind'
import { cardSchema, changesSchema, imageCreatedSchema } from './schemas'

const cardPath = (id: string) => `/api/cards/${encodeURIComponent(id)}`

export function listCards(signal?: AbortSignal): Promise<CardDto[]> {
  return getJson('/api/cards', z.array(cardSchema), signal)
}

/** Pull sync: every visible card, or `cards: null` when nothing changed since `since`. */
export function fetchChanges(since: number, signal?: AbortSignal): Promise<ChangesDto> {
  return getJson(`/api/changes?since=${since}`, changesSchema, signal)
}

/** Creates or replaces the card with this client-generated id. */
export function saveCard(id: string, input: CardInput): Promise<CardDto> {
  return putJson(cardPath(id), cardSchema, input)
}

export function deleteCard(id: string): Promise<void> {
  return sendRequest('DELETE', cardPath(id))
}

export function setFavorite(id: string, isFavorite: boolean): Promise<CardDto> {
  return putJson(`${cardPath(id)}/prefs`, cardSchema, { isFavorite })
}

/** Counts an opening of the card, for "recently used". */
export function recordUse(id: string): Promise<void> {
  return sendRequest('POST', `${cardPath(id)}/use`)
}

export function uploadImage(cardId: string, kind: ImageKind, photo: Blob): Promise<ImageCreated> {
  return putBlob(`${cardPath(cardId)}/images/${kind}`, imageCreatedSchema, photo)
}

export function deleteImage(cardId: string, kind: ImageKind): Promise<void> {
  return sendRequest('DELETE', `${cardPath(cardId)}/images/${kind}`)
}

export function imageUrl(imageId: string): string {
  return `/api/images/${encodeURIComponent(imageId)}`
}
