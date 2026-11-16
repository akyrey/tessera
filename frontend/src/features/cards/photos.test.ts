import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { MAX_PHOTO_BYTES, MAX_PHOTO_DIMENSION, PhotoError, preparePhoto } from './photos'

const drawImage = vi.fn()
const close = vi.fn()

function stubBitmap(width: number, height: number) {
  vi.stubGlobal('createImageBitmap', vi.fn().mockResolvedValue({ width, height, close }))
}

/** Each `toBlob` call answers with the next result. */
function stubEncoder(...results: (Blob | null)[]) {
  const toBlob = vi.spyOn(HTMLCanvasElement.prototype, 'toBlob').mockImplementation(function (callback) {
    callback(results.shift() ?? null)
  })
  return toBlob
}

const blob = (type: string, size = 10) => new Blob([new Uint8Array(size)], { type })
const photo = new File(['x'], 'card.heic', { type: 'image/heic' })

beforeEach(() => {
  vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue({ drawImage } as never)
})

afterEach(() => {
  drawImage.mockReset()
  close.mockReset()
})

describe('preparePhoto', () => {
  it('scales large photos down and prefers WebP', async () => {
    stubBitmap(4000, 3000)
    const toBlob = stubEncoder(blob('image/webp'))

    const result = await preparePhoto(photo)

    expect(result.type).toBe('image/webp')
    const canvas = toBlob.mock.contexts[0] as HTMLCanvasElement
    expect([canvas.width, canvas.height]).toEqual([MAX_PHOTO_DIMENSION, 960])
    expect(close).toHaveBeenCalled()
  })

  it('keeps small photos at their size and falls back to JPEG', async () => {
    stubBitmap(600, 400)
    const toBlob = stubEncoder(blob('image/png'), blob('image/jpeg'))

    const result = await preparePhoto(photo)

    expect(result.type).toBe('image/jpeg')
    expect(toBlob.mock.calls.map((call) => call[1])).toEqual(['image/webp', 'image/jpeg'])
    expect(drawImage).toHaveBeenCalledWith(expect.anything(), 0, 0, 600, 400)
  })

  it('lowers the quality until the photo is small enough', async () => {
    stubBitmap(800, 600)
    const toBlob = stubEncoder(blob('image/webp', MAX_PHOTO_BYTES + 1), blob('image/webp', 100))

    await expect(preparePhoto(photo)).resolves.toHaveProperty('size', 100)
    expect(toBlob.mock.calls.map((call) => call[2])).toEqual([0.82, 0.7])
  })

  it('gives up on photos that stay too large', async () => {
    stubBitmap(800, 600)
    const big = () => blob('image/webp', MAX_PHOTO_BYTES + 1)
    stubEncoder(big(), big(), big())

    await expect(preparePhoto(photo)).rejects.toThrow('too detailed')
  })

  it('rejects files that are not images or cannot be decoded', async () => {
    await expect(preparePhoto(new Blob(['%PDF'], { type: 'application/pdf' }))).rejects.toBeInstanceOf(
      PhotoError,
    )
    vi.stubGlobal('createImageBitmap', vi.fn().mockRejectedValue(new Error('bad')))
    await expect(preparePhoto(photo)).rejects.toThrow('cannot be read')
  })

  it('fails clearly when the browser cannot encode', async () => {
    stubBitmap(10, 10)
    stubEncoder(null, null)
    await expect(preparePhoto(photo)).rejects.toThrow('cannot process photos')
  })
})

describe('toDataUrl', () => {
  it('encodes the photo for previews', async () => {
    const { toDataUrl } = await import('./photos')
    await expect(toDataUrl(new Blob(['hi'], { type: 'image/webp' }))).resolves.toBe(
      'data:image/webp;base64,aGk=',
    )
  })
})
