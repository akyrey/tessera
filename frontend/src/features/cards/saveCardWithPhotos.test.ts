import { describe, expect, it, vi } from 'vitest'
import * as api from '../../api/cards'
import { card } from '../../test/fixtures'
import { KEEP_PHOTO } from './photoChange'
import { saveCardWithPhotos } from './saveCardWithPhotos'

vi.mock('../../api/cards')

const input = { storeName: 'Coop', cardNumber: '123', barcodeFormat: 'none' } as const

describe('saveCardWithPhotos', () => {
  it('saves the card and leaves photos alone by default', async () => {
    vi.mocked(api.saveCard).mockResolvedValue(card({ frontImageId: 'f-1' }))

    const saved = await saveCardWithPhotos('c1', input, { front: KEEP_PHOTO, back: KEEP_PHOTO })

    expect(api.saveCard).toHaveBeenCalledWith('c1', input)
    expect(saved.frontImageId).toBe('f-1')
    expect(api.uploadImage).not.toHaveBeenCalled()
  })

  it('uploads new photos and removes deleted ones', async () => {
    const photo = new Blob(['x'], { type: 'image/webp' })
    vi.mocked(api.saveCard).mockResolvedValue(card({ id: 'c1', backImageId: 'b-1' }))
    vi.mocked(api.uploadImage).mockResolvedValue({ id: 'f-2' })
    vi.mocked(api.deleteImage).mockResolvedValue()

    const saved = await saveCardWithPhotos('c1', input, {
      front: { type: 'replace', photo, preview: 'data:' },
      back: { type: 'remove' },
    })

    expect(api.uploadImage).toHaveBeenCalledWith('c1', 'front', photo)
    expect(api.deleteImage).toHaveBeenCalledWith('c1', 'back')
    expect(saved).toMatchObject({ frontImageId: 'f-2', backImageId: null })
  })
})
