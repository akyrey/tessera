import { useLiveQuery } from 'dexie-react-hooks'
import { Suspense, lazy, useMemo } from 'react'
import { Link, useNavigate } from 'react-router'
import { Button } from '../../components/Button'
import { FullPageSpinner } from '../../components/Spinner'
import { db } from '../../offline/db'
import { useCurrentPosition } from '../nearby/useCurrentPosition'
import { shopMarkers } from './mapMarkers'

/** Leaflet loads only when the map is opened. */
const ShopMap = lazy(() => import('./ShopMap'))

export function MapPage() {
  const navigate = useNavigate()
  const cards = useLiveQuery(() => db.cards.toArray(), [])
  const locations = useLiveQuery(() => db.locations.toArray(), [])
  const { state, locate } = useCurrentPosition()
  const markers = useMemo(() => shopMarkers(cards ?? [], locations ?? []), [cards, locations])
  const position = state.status === 'ready' ? state.position : null

  return (
    <main className="mx-auto flex h-dvh w-full max-w-md flex-col">
      <header className="flex items-center justify-between gap-2 px-4 pt-[max(1rem,env(safe-area-inset-top))] pb-2">
        <Link to="/" className="min-h-11 py-2 font-medium text-brand-700">
          ‹ Cards
        </Link>
        <h1 className="text-lg font-semibold">Shops</h1>
        {state.status === 'idle' || state.status === 'locating' ? (
          <Button variant="secondary" busy={state.status === 'locating'} onClick={locate}>
            Where am I?
          </Button>
        ) : (
          <span className="w-11" />
        )}
      </header>
      {markers.length === 0 && cards !== undefined && (
        <p className="px-4 pb-2 text-sm text-brand-900/60">
          No shops yet: link a brand or mark a shop on a card to see it here.
        </p>
      )}
      <Suspense fallback={<FullPageSpinner label="Loading map…" />}>
        <ShopMap markers={markers} position={position} onOpenCard={(id) => void navigate(`/cards/${id}`)} />
      </Suspense>
    </main>
  )
}
