import { useState } from 'react'
import type { Place } from '../../api/generated/Place'
import { Button } from '../../components/Button'
import type { LocalCard } from '../../offline/db'
import { enqueue } from '../../offline/syncEngine'
import { toInput } from './cardInput'

/** Matches the server's limit per card. */
const MAX_PLACES = 20
const LOCATE_TIMEOUT_MS = 15_000

/** Shops marked by hand, for chains OpenStreetMap does not know. */
export function PlacesSection({ card }: { readonly card: LocalCard }) {
  const [locating, setLocating] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const save = (places: readonly Place[]) =>
    enqueue(card.id, {
      type: 'save',
      input: { ...toInput(card), places: [...places] },
      brandName: card.brandName,
    })

  const markHere = () => {
    if (!('geolocation' in navigator)) return setError('Location is not available on this device.')
    setLocating(true)
    setError(null)
    navigator.geolocation.getCurrentPosition(
      ({ coords }) => {
        setLocating(false)
        void save([...card.places, { label: card.storeName, lat: coords.latitude, lon: coords.longitude }])
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
                onClick={() => void save(card.places.filter((_, i) => i !== index))}
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
