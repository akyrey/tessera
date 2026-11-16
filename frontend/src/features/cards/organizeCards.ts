import type { CardDto } from '../../api/generated/CardDto'

/** How many recently used cards get their own section. */
export const RECENT_LIMIT = 3

export interface OrganizedCards {
  readonly favorites: readonly CardDto[]
  readonly recent: readonly CardDto[]
  readonly others: readonly CardDto[]
}

const collator = new Intl.Collator('it', { sensitivity: 'base' })

/** Lower-case, accent-free text for forgiving search ("Crai" finds "CRAÌ"). */
export function searchKey(text: string): string {
  return text
    .normalize('NFD')
    .replace(/\p{Diacritic}/gu, '')
    .toLowerCase()
}

export function matchesQuery(card: CardDto, query: string): boolean {
  const needle = searchKey(query.trim())
  if (needle === '') return true
  return [card.storeName, card.memberName ?? '', card.notes ?? ''].some((field) =>
    searchKey(field).includes(needle),
  )
}

export function byStoreName(a: CardDto, b: CardDto): number {
  return collator.compare(a.storeName, b.storeName)
}

/**
 * Favourites first, then the most recently used cards, then everything else
 * alphabetically. Each card appears once.
 */
export function organizeCards(cards: readonly CardDto[]): OrganizedCards {
  const favorites = cards.filter((card) => card.isFavorite).toSorted(byStoreName)
  const recent = cards
    .filter((card) => !card.isFavorite && card.lastUsedAt !== null)
    .toSorted((a, b) => (b.lastUsedAt ?? 0) - (a.lastUsedAt ?? 0))
    .slice(0, RECENT_LIMIT)
  const placed = new Set([...favorites, ...recent].map((card) => card.id))
  const others = cards.filter((card) => !placed.has(card.id)).toSorted(byStoreName)
  return { favorites, recent, others }
}
