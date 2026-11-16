import { deleteImage, saveCard, uploadImage } from '../../api/cards'
import type { CardDto } from '../../api/generated/CardDto'
import type { CardInput } from '../../api/generated/CardInput'
import type { ImageKind } from '../../api/generated/ImageKind'
import type { PhotoChange } from './photoChange'

export type PhotoChanges = Readonly<Record<ImageKind, PhotoChange>>

const IMAGE_FIELD = { front: 'frontImageId', back: 'backImageId' } as const satisfies Record<
  ImageKind,
  keyof CardDto
>

/**
 * Saves the card, then applies photo changes one by one. The card id is chosen
 * by the client, so retrying after a partial failure is safe.
 */
export async function saveCardWithPhotos(
  id: string,
  input: CardInput,
  photos: PhotoChanges,
): Promise<CardDto> {
  let card = await saveCard(id, input)
  for (const kind of ['front', 'back'] as const) {
    card = { ...card, [IMAGE_FIELD[kind]]: await applyPhoto(card, kind, photos[kind]) }
  }
  return card
}

async function applyPhoto(card: CardDto, kind: ImageKind, change: PhotoChange): Promise<string | null> {
  switch (change.type) {
    case 'keep':
      return card[IMAGE_FIELD[kind]]
    case 'remove':
      await deleteImage(card.id, kind)
      return null
    case 'replace':
      return (await uploadImage(card.id, kind, change.photo)).id
  }
}
