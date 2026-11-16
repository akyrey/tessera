import type { CardInput } from '../api/generated/CardInput'
import type { CardOp, LocalCard } from './db'

/** Unix seconds, the unit the API uses. */
export const nowSeconds = (): number => Math.floor(Date.now() / 1000)

/**
 * What the card looks like on this device after `op`, before the server has
 * seen it. `undefined` means the card is gone.
 */
export function applyOp(
  card: LocalCard | undefined,
  op: CardOp,
  context: { readonly cardId: string; readonly userId: string; readonly now: number },
): LocalCard | undefined {
  switch (op.type) {
    case 'save':
      return card ? { ...card, ...fieldsOf(op.input), updatedAt: context.now } : newCard(context, op.input)
    case 'delete':
      return undefined
    case 'favorite':
      return card && { ...card, isFavorite: op.isFavorite }
    case 'use':
      return card && { ...card, useCount: card.useCount + 1, lastUsedAt: op.at }
    case 'photo':
      return card && { ...card, localPhotos: { ...card.localPhotos, [op.kind]: op.preview } }
  }
}

function fieldsOf(input: CardInput) {
  return {
    storeName: input.storeName,
    cardNumber: input.cardNumber,
    barcodeFormat: input.barcodeFormat,
    memberName: input.memberName ?? null,
    color: input.color ?? null,
    notes: input.notes ?? null,
  }
}

function newCard(context: { cardId: string; userId: string; now: number }, input: CardInput): LocalCard {
  return {
    id: context.cardId,
    ownerId: context.userId,
    ...fieldsOf(input),
    isFavorite: false,
    useCount: 0,
    lastUsedAt: null,
    frontImageId: null,
    backImageId: null,
    createdAt: context.now,
    updatedAt: context.now,
  }
}
