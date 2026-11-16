import { describe, expect, it } from 'vitest'
import { card } from '../../test/fixtures'
import { RECENT_LIMIT, matchesQuery, organizeCards, searchKey } from './organizeCards'

const named = (id: string, storeName: string, extra: Parameters<typeof card>[0] = {}) =>
  card({ id, storeName, ...extra })
const ids = (cards: readonly { id: string }[]) => cards.map((c) => c.id)

describe('organizeCards', () => {
  it('puts favourites first, then recent cards, then the rest alphabetically', () => {
    const cards = [
      named('z', 'Zara'),
      named('fav', 'Coop', { isFavorite: true, lastUsedAt: 50 }),
      named('old', 'Bennet', { lastUsedAt: 10 }),
      named('new', 'Decathlon', { lastUsedAt: 30 }),
      named('a', 'auchan'),
    ]

    const result = organizeCards(cards)

    expect(ids(result.favorites)).toEqual(['fav'])
    expect(ids(result.recent)).toEqual(['new', 'old'])
    expect(ids(result.others)).toEqual(['a', 'z'])
  })

  it(`keeps only the ${RECENT_LIMIT} most recent cards in the recent section`, () => {
    const cards = [1, 2, 3, 4].map((n) => named(`c${n}`, `Shop ${n}`, { lastUsedAt: n }))

    const result = organizeCards(cards)

    expect(ids(result.recent)).toEqual(['c4', 'c3', 'c2'])
    expect(ids(result.others)).toEqual(['c1'])
  })

  it('lifts coupons expiring soon above everything else', () => {
    const today = new Date(2027, 2, 10)
    const cards = [
      named('later', 'Later', { expiresOn: '2027-03-15' }),
      named('first', 'First', { expiresOn: '2027-03-11', isFavorite: true }),
      named('far', 'Far', { expiresOn: '2027-12-31' }),
      named('gone', 'Gone', { expiresOn: '2027-03-01' }),
    ]

    const result = organizeCards(cards, today)

    expect(ids(result.expiring)).toEqual(['first', 'later'])
    expect(ids(result.favorites)).toEqual([])
    expect(ids(result.others)).toEqual(['far', 'gone'])
  })

  it('handles an empty wallet', () => {
    expect(organizeCards([])).toEqual({ expiring: [], favorites: [], recent: [], others: [] })
  })
})

describe('search', () => {
  it('ignores case and accents', () => {
    expect(searchKey('CRAÌ Più')).toBe('crai piu')
    expect(matchesQuery(named('1', 'CRAÌ'), ' crai ')).toBe(true)
  })

  it('matches member name and notes, and everything on an empty query', () => {
    const c = named('1', 'Coop', { memberName: 'Bea', notes: 'Socio dal 2020' })
    expect(matchesQuery(c, 'bea')).toBe(true)
    expect(matchesQuery(c, 'socio')).toBe(true)
    expect(matchesQuery(c, 'esselunga')).toBe(false)
    expect(matchesQuery(c, '  ')).toBe(true)
  })
})
