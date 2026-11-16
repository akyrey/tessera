import { describe, expect, it } from 'vitest'
import { CARD_COLORS, cardColor, textOn } from './cardColors'

describe('cardColor', () => {
  it('uses the chosen colour', () => {
    expect(cardColor({ color: '#ffcc00', storeName: 'Coop' })).toBe('#ffcc00')
  })

  it('derives a stable palette colour from the store name', () => {
    const first = cardColor({ color: null, storeName: 'Esselunga' })
    expect(CARD_COLORS).toContain(first)
    expect(cardColor({ color: null, storeName: 'esselunga' })).toBe(first)
  })
})

describe('textOn', () => {
  it('picks dark text on light colours and white on dark ones', () => {
    expect(textOn('#ffcc00')).toBe('#111827')
    expect(textOn('#ffffff')).toBe('#111827')
    expect(textOn('#0f766e')).toBe('#ffffff')
    expect(textOn('#000000')).toBe('#ffffff')
  })
})
