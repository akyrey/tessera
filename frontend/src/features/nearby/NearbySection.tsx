import { useLiveQuery } from 'dexie-react-hooks'
import { useMemo } from 'react'
import { Button } from '../../components/Button'
import { type LocalCard, db, readMeta } from '../../offline/db'
import { CardTile } from '../cards/CardTile'
import { findNearby, formatDistance } from './nearby'
import { useCurrentPosition } from './useCurrentPosition'

/** Default until the server's setting has been synced. */
const DEFAULT_RADIUS_M = 200

interface NearbySectionProps {
  readonly cards: readonly LocalCard[]
  readonly userId: string | undefined
}

/** Cards for shops around the user, computed on the phone from one location fix. */
export function NearbySection({ cards, userId }: NearbySectionProps) {
  const { state, locate } = useCurrentPosition()
  const locations = useLiveQuery(() => db.locations.toArray(), [])
  const radiusM = useLiveQuery(() => readMeta('radiusM'), []) ?? DEFAULT_RADIUS_M
  const hasShops = cards.some((card) => card.brandId !== null || card.places.length > 0)

  const nearby = useMemo(() => {
    if (state.status !== 'ready' || !locations) return []
    const points = new Map(locations.map((entry) => [entry.brandId, entry.points]))
    return findNearby(cards, points, state.position, radiusM)
  }, [state, locations, cards, radiusM])

  if (!hasShops || state.status === 'unavailable' || state.status === 'denied') return null
  if (state.status === 'idle' || state.status === 'locating') {
    return (
      <Button variant="secondary" busy={state.status === 'locating'} onClick={locate}>
        Show cards for shops near me
      </Button>
    )
  }
  if (nearby.length === 0) return null
  return (
    <section aria-label="Nearby" className="flex flex-col gap-2">
      <h2 className="text-sm font-semibold tracking-wide text-brand-900/60 uppercase">Nearby</h2>
      <ul className="grid grid-cols-2 gap-3">
        {nearby.map(({ card, distanceM }) => (
          <li key={card.id} className="flex flex-col gap-1">
            <CardTile card={card} userId={userId} />
            <span className="text-center text-xs text-brand-900/60">{formatDistance(distanceM)}</span>
          </li>
        ))}
      </ul>
    </section>
  )
}
