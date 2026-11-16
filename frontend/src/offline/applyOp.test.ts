import { describe, expect, it } from 'vitest'
import { card } from '../test/fixtures'
import { applyOp } from './applyOp'

const context = { cardId: 'c-1', userId: 'u-alice', userName: 'Alice', now: 1_900_000_000 }
const input = { storeName: 'Coop', cardNumber: '123', barcodeFormat: 'none', notes: 'n' } as const

describe('applyOp', () => {
  it('creates a card owned by the current user', () => {
    expect(applyOp(undefined, { type: 'save', input }, context)).toMatchObject({
      id: 'c-1',
      ownerId: 'u-alice',
      ownerName: 'Alice',
      shared: false,
      storeName: 'Coop',
      notes: 'n',
      memberName: null,
      useCount: 0,
      createdAt: context.now,
    })
  })

  it('updates fields and keeps preferences and photos', () => {
    const existing = card({ isFavorite: true, frontImageId: 'img', memberName: 'Alice' })
    const updated = applyOp(existing, { type: 'save', input }, context)
    expect(updated).toMatchObject({
      storeName: 'Coop',
      memberName: null,
      isFavorite: true,
      frontImageId: 'img',
    })
    expect(existing.storeName).toBe('Esselunga')
  })

  it('carries the brand and places of a save', () => {
    const place = { label: 'Via Roma', lat: 45, lon: 9 }
    const saved = applyOp(
      undefined,
      { type: 'save', input: { ...input, brandId: 'b1', places: [place] }, brandName: 'Coop' },
      context,
    )
    expect(saved).toMatchObject({ brandId: 'b1', brandName: 'Coop', places: [place] })
    // A save without places keeps them; places have their own op.
    expect(applyOp(saved, { type: 'save', input }, context)).toMatchObject({ brandId: null, places: [place] })
    expect(applyOp(saved, { type: 'places', places: [] }, context)?.places).toEqual([])
  })

  it('keeps the brand logo only while the brand stays the same', () => {
    const existing = card({ brandId: 'b1', brandName: 'Coop', brandLogoId: 'logo-1' })
    const same = applyOp(
      existing,
      { type: 'save', input: { ...input, brandId: 'b1' }, brandName: 'Coop' },
      context,
    )
    expect(same?.brandLogoId).toBe('logo-1')
    const other = applyOp(
      existing,
      { type: 'save', input: { ...input, brandId: 'b2' }, brandName: 'Conad' },
      context,
    )
    expect(other?.brandLogoId).toBeNull()
    expect(applyOp(undefined, { type: 'save', input }, context)?.brandLogoId).toBeNull()
  })

  it('handles deletion, favourites, usage and photos', () => {
    const existing = card({ useCount: 2 })
    expect(applyOp(existing, { type: 'delete' }, context)).toBeUndefined()
    expect(applyOp(existing, { type: 'favorite', isFavorite: true }, context)?.isFavorite).toBe(true)
    expect(applyOp(existing, { type: 'share', shared: true }, context)?.shared).toBe(true)
    expect(applyOp(existing, { type: 'use', at: 5 }, context)).toMatchObject({ useCount: 3, lastUsedAt: 5 })
    const photo = { type: 'photo', kind: 'back', photo: null, preview: null } as const
    expect(applyOp(existing, photo, context)?.localPhotos).toEqual({ back: null })
  })

  it('ignores changes to cards that are gone', () => {
    expect(applyOp(undefined, { type: 'favorite', isFavorite: true }, context)).toBeUndefined()
    expect(applyOp(undefined, { type: 'use', at: 1 }, context)).toBeUndefined()
  })
})
