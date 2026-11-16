import type { CardInput } from '../../api/generated/CardInput'
import type { LocalCard } from '../../offline/db'

/** The save request that reproduces `card` as it is (to change one part of it). */
export function toInput(card: LocalCard): CardInput {
  return {
    storeName: card.storeName,
    cardNumber: card.cardNumber,
    barcodeFormat: card.barcodeFormat,
    memberName: card.memberName ?? undefined,
    color: card.color ?? undefined,
    notes: card.notes ?? undefined,
    brandId: card.brandId ?? undefined,
    places: card.places,
  }
}
