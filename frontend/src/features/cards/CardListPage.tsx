import { useState } from 'react'
import { Link } from 'react-router'
import type { CardDto } from '../../api/generated/CardDto'
import { Alert } from '../../components/Alert'
import { Button } from '../../components/Button'
import { Screen } from '../../components/Screen'
import { describeError } from '../auth/errors'
import { useMe } from '../auth/useMe'
import { CardTile } from './CardTile'
import { byStoreName, matchesQuery, organizeCards } from './organizeCards'
import { useCards } from './useCards'

const LINK_BUTTON =
  'inline-flex min-h-11 items-center justify-center rounded-xl px-4 py-2.5 font-medium transition-colors'

export function CardListPage() {
  const me = useMe().data
  const cards = useCards()
  const [query, setQuery] = useState('')

  return (
    <Screen>
      <header className="flex items-center justify-between gap-3">
        <div className="min-w-0">
          <p className="truncate text-sm text-brand-900/60">{me?.household.name}</p>
          <h1 className="text-2xl font-semibold">Cards</h1>
        </div>
        <Link
          to="/settings"
          className={`${LINK_BUTTON} text-brand-700 ring-1 ring-brand-900/15 hover:bg-white`}
        >
          Settings
        </Link>
      </header>

      {cards.isPending && <p role="status">Loading cards…</p>}
      {cards.isError && (
        <div className="flex flex-col gap-3">
          <Alert>{describeError(cards.error)}</Alert>
          <Button variant="secondary" onClick={() => void cards.refetch()}>
            Try again
          </Button>
        </div>
      )}
      {cards.isSuccess &&
        (cards.data.length === 0 ? (
          <EmptyWallet />
        ) : (
          <>
            <input
              type="search"
              aria-label="Search cards"
              placeholder="Search cards"
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              className="min-h-11 rounded-xl bg-white px-4 ring-1 ring-brand-900/15 focus:ring-2 focus:ring-brand-700 focus:outline-none"
            />
            {query.trim() ? (
              <SearchResults cards={cards.data} query={query} />
            ) : (
              <Wallet cards={cards.data} />
            )}
          </>
        ))}

      <Link
        to="/cards/new"
        className={`${LINK_BUTTON} sticky bottom-[max(1rem,env(safe-area-inset-bottom))] mt-auto bg-brand-700 text-white shadow-lg hover:bg-brand-900`}
      >
        Add card
      </Link>
    </Screen>
  )
}

function Wallet({ cards }: { readonly cards: readonly CardDto[] }) {
  const { favorites, recent, others } = organizeCards(cards)
  return (
    <>
      <CardGroup title="Favourites" cards={favorites} />
      <CardGroup title="Recently used" cards={recent} />
      <CardGroup
        title={favorites.length + recent.length > 0 ? 'All other cards' : 'All cards'}
        cards={others}
      />
    </>
  )
}

function SearchResults({ cards, query }: { readonly cards: readonly CardDto[]; readonly query: string }) {
  const matches = cards.filter((card) => matchesQuery(card, query)).toSorted(byStoreName)
  if (matches.length === 0)
    return <p className="text-center text-brand-900/60">No cards match “{query.trim()}”.</p>
  return <CardGroup title="Search results" cards={matches} />
}

function CardGroup({ title, cards }: { readonly title: string; readonly cards: readonly CardDto[] }) {
  if (cards.length === 0) return null
  return (
    <section aria-label={title} className="flex flex-col gap-2">
      <h2 className="text-sm font-semibold tracking-wide text-brand-900/60 uppercase">{title}</h2>
      <ul className="grid grid-cols-2 gap-3">
        {cards.map((card) => (
          <li key={card.id}>
            <CardTile card={card} />
          </li>
        ))}
      </ul>
    </section>
  )
}

function EmptyWallet() {
  return (
    <section className="flex flex-1 flex-col items-center justify-center gap-2 rounded-2xl border-2 border-dashed border-brand-900/15 p-8 text-center">
      <h2 className="text-lg font-semibold">No cards yet</h2>
      <p className="text-brand-900/60">
        Add your first loyalty card: you only need the number under its barcode.
      </p>
    </section>
  )
}
