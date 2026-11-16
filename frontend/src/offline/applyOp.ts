import type { CardInput } from '../api/generated/CardInput'
import type { CardOp, LocalCard } from './db'

/** Unix seconds, the unit the API uses. */
export const nowSeconds = (): number => Math.floor(Date.now() / 1000)

export interface OpContext {
  readonly cardId: string
  readonly userId: string
  readonly userName: string
  readonly now: number
}

/**
 * What the card looks like on this device after `op`, before the server has
 * seen it. `undefined` means the card is gone.
 */
export function applyOp(card: LocalCard | undefined, op: CardOp, context: OpContext): LocalCard | undefined {
  switch (op.type) {
    case 'save':
      return card
        ? {
            ...card,
            ...fieldsOf(op.input, op.brandName),
            places: op.input.places ?? card.places,
            updatedAt: context.now,
          }
        : newCard(context, op.input, op.brandName)
    case 'delete':
      return undefined
    case 'favorite':
      return card && { ...card, isFavorite: op.isFavorite }
    case 'share':
      return card && { ...card, shared: op.shared }
    case 'places':
      return card && { ...card, places: [...op.places] }
    case 'use':
      return card && { ...card, useCount: card.useCount + 1, lastUsedAt: op.at }
    case 'photo':
      return card && { ...card, localPhotos: { ...card.localPhotos, [op.kind]: op.preview } }
  }
}

function fieldsOf(input: CardInput, brandName: string | null | undefined) {
  return {
    brandId: input.brandId ?? null,
    brandName: input.brandId ? (brandName ?? null) : null,
    places: input.places ?? [],
    expiresOn: input.expiresOn ?? null,
    storeName: input.storeName,
    cardNumber: input.cardNumber,
    barcodeFormat: input.barcodeFormat,
    memberName: input.memberName ?? null,
    color: input.color ?? null,
    notes: input.notes ?? null,
  }
}

function newCard(context: OpContext, input: CardInput, brandName: string | null | undefined): LocalCard {
  return {
    id: context.cardId,
    ownerId: context.userId,
    ownerName: context.userName,
    shared: false,
    ...fieldsOf(input, brandName),
    isFavorite: false,
    useCount: 0,
    lastUsedAt: null,
    frontImageId: null,
    backImageId: null,
    createdAt: context.now,
    updatedAt: context.now,
  }
}
