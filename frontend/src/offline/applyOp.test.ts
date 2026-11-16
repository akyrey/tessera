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
