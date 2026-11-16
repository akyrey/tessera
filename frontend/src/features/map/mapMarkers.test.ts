import { describe, expect, it } from 'vitest'
import { card } from '../../test/fixtures'
import { shopMarkers } from './mapMarkers'

describe('shopMarkers', () => {
  it('lists brand shops and hand-marked places per card', () => {
    const cards = [
      card({ id: 'a', storeName: 'Esselunga', brandId: 'b1' }),
      card({ id: 'b', storeName: 'Forno', places: [{ label: 'Forno', lat: 1, lon: 2 }] }),
      card({ id: 'c', storeName: 'Unknown brand', brandId: 'missing' }),
    ]
    const markers = shopMarkers(cards, [
      {
        brandId: 'b1',
        points: [
          [45, 9],
          [46, 10],
        ],
      },
    ])

    expect(markers).toEqual([
      { lat: 45, lon: 9, cardId: 'a', storeName: 'Esselunga', source: 'brand' },
      { lat: 46, lon: 10, cardId: 'a', storeName: 'Esselunga', source: 'brand' },
      { lat: 1, lon: 2, cardId: 'b', storeName: 'Forno', source: 'place' },
    ])
  })
})
