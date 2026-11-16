import { imageUrl } from '../../api/cards'
import type { ImageKind } from '../../api/generated/ImageKind'
import type { LocalCard } from '../../offline/db'

/** The photo to show: one changed on this device (not yet synced), else the server's. */
export function photoSrc(card: LocalCard, kind: ImageKind): string | null {
  const local = card.localPhotos?.[kind]
  if (local !== undefined) return local
  const imageId = kind === 'front' ? card.frontImageId : card.backImageId
  return imageId ? imageUrl(imageId) : null
}
