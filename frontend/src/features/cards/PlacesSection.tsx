import { useState } from 'react'
import type { Place } from '../../api/generated/Place'
import { Button } from '../../components/Button'
import { type LocalCard, db } from '../../offline/db'
import { enqueue } from '../../offline/syncEngine'

/** Matches the server's limit per card. */
const MAX_PLACES = 20
const LOCATE_TIMEOUT_MS = 15_000

/** Shops marked by hand, for chains OpenStreetMap does not know. */
export function PlacesSection({ card }: { readonly card: LocalCard }) {
  const [locating, setLocating] = useState(false)
  const [error, setError] = useState<string | null>(null)

  /** Changes the latest stored places (not this render's copy, which may be stale by now). */
  const update = async (change: (places: readonly Place[]) => readonly Place[]) => {
    const latest = (await db.cards.get(card.id))?.places ?? card.places
    await enqueue(card.id, { type: 'places', places: change(latest) })
  }

  const markHere = () => {
    if (!('geolocation' in navigator)) return setError('Location is not available on this device.')
    setLocating(true)
    setError(null)
    navigator.geolocation.getCurrentPosition(
      ({ coords }) => {
        setLocating(false)
        void update((places) => [
          ...places,
          { label: card.storeName, lat: coords.latitude, lon: coords.longitude },
        ])
      },
      (failure) => {
        setLocating(false)
        setError(
          failure.code === failure.PERMISSION_DENIED
            ? 'Location access was denied. Allow it in the browser settings to mark this shop.'
            : 'Your location could not be found. Try again outdoors or near a window.',
        )
      },
      { enableHighAccuracy: true, timeout: LOCATE_TIMEOUT_MS, maximumAge: 0 },
    )
  }

  return (
    <section aria-label="Shop locations" className="flex flex-col gap-2">
      <h2 className="font-semibold">Shop locations</h2>
      {card.places.length > 0 && (
        <ul className="flex flex-col divide-y divide-brand-900/10">
          {card.places.map((place, index) => (
            <li
              key={`${place.lat},${place.lon},${index}`}
              className="flex items-center justify-between gap-2 py-1"
            >
              <span className="truncate text-sm">
                {place.label} · {place.lat.toFixed(4)}, {place.lon.toFixed(4)}
              </span>
              <Button
                variant="secondary"
                aria-label={`Remove location ${index + 1}`}
                onClick={() => void update((places) => places.filter((_, i) => i !== index))}
              >
                Remove
              </Button>
            </li>
          ))}
        </ul>
      )}
      <Button
        variant="secondary"
        busy={locating}
        disabled={card.places.length >= MAX_PLACES}
        onClick={markHere}
      >
        I am at this shop: remember its location
      </Button>
      {error && (
        <p role="alert" className="text-sm text-red-700">
          {error}
        </p>
      )}
      <p className="text-sm text-brand-900/60">
        {card.brandName
          ? `Shops of ${card.brandName} are found automatically; mark the ones that are missing.`
          : 'Marked shops make this card appear at the top when you are there.'}
      </p>
    </section>
  )
}
