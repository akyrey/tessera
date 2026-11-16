/** Longest side of a stored photo: plenty to read a card, small enough to sync. */
export const MAX_PHOTO_DIMENSION = 1280
/** Keep well under the server's 512 KB limit. */
export const MAX_PHOTO_BYTES = 400 * 1024
const QUALITIES = [0.82, 0.7, 0.55] as const

export class PhotoError extends Error {
  constructor(message: string) {
    super(message)
    this.name = 'PhotoError'
  }
}

/**
 * Downscales and re-encodes a picked photo (WebP where the browser can encode
 * it, JPEG otherwise, e.g. Safari). Also strips EXIF metadata such as GPS.
 */
export async function preparePhoto(file: Blob): Promise<Blob> {
  if (!file.type.startsWith('image/')) throw new PhotoError('Please choose an image file.')
  let bitmap: ImageBitmap
  try {
    bitmap = await createImageBitmap(file)
  } catch {
    throw new PhotoError('This image cannot be read. Try a JPEG or PNG photo.')
  }
  try {
    const canvas = drawScaled(bitmap)
    for (const quality of QUALITIES) {
      const blob = await encode(canvas, quality)
      if (blob.size <= MAX_PHOTO_BYTES) return blob
    }
    throw new PhotoError('This photo is too detailed to store. Try a closer, simpler shot.')
  } finally {
    bitmap.close()
  }
}

function drawScaled(bitmap: ImageBitmap): HTMLCanvasElement {
  const scale = Math.min(1, MAX_PHOTO_DIMENSION / Math.max(bitmap.width, bitmap.height))
  const canvas = document.createElement('canvas')
  canvas.width = Math.max(1, Math.round(bitmap.width * scale))
  canvas.height = Math.max(1, Math.round(bitmap.height * scale))
  const context = canvas.getContext('2d')
  if (!context) throw new PhotoError('Your browser cannot process photos.')
  context.drawImage(bitmap, 0, 0, canvas.width, canvas.height)
  return canvas
}

async function encode(canvas: HTMLCanvasElement, quality: number): Promise<Blob> {
  const webp = await toBlob(canvas, 'image/webp', quality)
  // Browsers that cannot encode WebP silently fall back to PNG.
  if (webp?.type === 'image/webp') return webp
  const jpeg = await toBlob(canvas, 'image/jpeg', quality)
  if (!jpeg) throw new PhotoError('Your browser cannot process photos.')
  return jpeg
}

function toBlob(canvas: HTMLCanvasElement, type: string, quality: number): Promise<Blob | null> {
  return new Promise((resolve) => canvas.toBlob(resolve, type, quality))
}

/** A `data:` URL for previewing a prepared photo (allowed by the CSP, nothing to revoke). */
export function toDataUrl(photo: Blob): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader()
    reader.onload = () => resolve(String(reader.result))
    reader.onerror = () => reject(new PhotoError('This photo could not be processed.'))
    reader.readAsDataURL(photo)
  })
}
