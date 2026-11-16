import type { CardDto } from '../../api/generated/CardDto'

/** Colours offered in the card form; "auto" picks one from the store name. */
export const CARD_COLORS = [
  '#0f766e',
  '#1d4ed8',
  '#7c3aed',
  '#be123c',
  '#c2410c',
  '#a16207',
  '#15803d',
  '#334155',
] as const

/** Contrast threshold (relative luminance) above which dark text reads better. */
const LIGHT_BACKGROUND_LUMINANCE = 0.4

export function cardColor(card: Pick<CardDto, 'color' | 'storeName'>): string {
  if (card.color) return card.color
  const hash = [...card.storeName.toLowerCase()].reduce(
    (sum, char) => (sum * 31 + char.charCodeAt(0)) >>> 0,
    7,
  )
  return CARD_COLORS[hash % CARD_COLORS.length] ?? CARD_COLORS[0]
}

/** Text colour that stays readable on `background` (`#rrggbb`). */
export function textOn(background: string): string {
  const channels = [1, 3, 5].map((start) => {
    const value = parseInt(background.slice(start, start + 2), 16) / 255
    return value <= 0.03928 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4
  })
  const [r = 0, g = 0, b = 0] = channels
  const luminance = 0.2126 * r + 0.7152 * g + 0.0722 * b
  return luminance > LIGHT_BACKGROUND_LUMINANCE ? '#111827' : '#ffffff'
}
