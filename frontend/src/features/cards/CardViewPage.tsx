import { useMutation, useQueryClient } from '@tanstack/react-query'
import { useEffect, useRef } from 'react'
import { Link } from 'react-router'
import { imageUrl, recordUse, setFavorite } from '../../api/cards'
import type { CardDto } from '../../api/generated/CardDto'
import { Alert } from '../../components/Alert'
import { describeError } from '../auth/errors'
import { Barcode } from './Barcode'
import { CardStatus } from './CardStatus'
import { cardColor, textOn } from './cardColors'
import { CARDS_QUERY_KEY, storeCard } from './useCards'
import { useCardFromRoute } from './useCardFromRoute'
import { useWakeLock } from './useWakeLock'

export function CardViewPage() {
  const { card, query } = useCardFromRoute()
  if (!card) return <CardStatus query={query} />
  return <CardView card={card} />
}

function CardView({ card }: { readonly card: CardDto }) {
  const queryClient = useQueryClient()
  useWakeLock()
  useRecordUse(card.id)

  const favorite = useMutation({
    mutationFn: () => setFavorite(card.id, !card.isFavorite),
    onSuccess: (saved) => storeCard(queryClient, saved),
  })
  const background = cardColor(card)
  const foreground = textOn(background)

  return (
    <main className="mx-auto flex min-h-dvh w-full max-w-md flex-col bg-white pb-[max(1.5rem,env(safe-area-inset-bottom))]">
      <header
        className="flex flex-col gap-4 px-4 pt-[max(1rem,env(safe-area-inset-top))] pb-6"
        style={{ backgroundColor: background, color: foreground }}
      >
        <nav className="flex items-center justify-between">
          <Link to="/" className="min-h-11 py-2 font-medium">
            ‹ Cards
          </Link>
          <div className="flex items-center gap-1">
            <button
              type="button"
              aria-pressed={card.isFavorite}
              aria-label="Favourite"
              disabled={favorite.isPending}
              onClick={() => favorite.mutate()}
              className="min-h-11 min-w-11 rounded-xl text-2xl"
            >
              {card.isFavorite ? '★' : '☆'}
            </button>
            <Link to={`/cards/${card.id}/edit`} className="min-h-11 rounded-xl px-3 py-2 font-medium">
              Edit
            </Link>
          </div>
        </nav>
        <h1 className="text-3xl font-semibold break-words">{card.storeName}</h1>
        {card.memberName && <p className="opacity-80">{card.memberName}</p>}
      </header>

      <section aria-label="Barcode" className="flex flex-col items-center gap-4 px-4 py-8 text-black">
        <Barcode format={card.barcodeFormat} value={card.cardNumber} />
        <p className="text-center font-mono text-2xl tracking-wider break-all select-all">
          {card.cardNumber}
        </p>
      </section>

      <div className="flex flex-col gap-4 px-4">
        {favorite.isError && <Alert>{describeError(favorite.error)}</Alert>}
        {card.notes && <p className="whitespace-pre-line text-brand-900/80">{card.notes}</p>}
        <Photo label="Front of the card" imageId={card.frontImageId} />
        <Photo label="Back of the card" imageId={card.backImageId} />
      </div>
    </main>
  )
}

function Photo({ label, imageId }: { readonly label: string; readonly imageId: string | null }) {
  if (!imageId) return null
  return (
    <img
      src={imageUrl(imageId)}
      alt={label}
      loading="lazy"
      className="w-full rounded-2xl ring-1 ring-black/10"
    />
  )
}

/** Counts one use per card opening (also under StrictMode's double effects). */
function useRecordUse(cardId: string): void {
  const queryClient = useQueryClient()
  const recorded = useRef<string | null>(null)
  useEffect(() => {
    if (recorded.current === cardId) return
    recorded.current = cardId
    recordUse(cardId)
      .then(() => queryClient.invalidateQueries({ queryKey: CARDS_QUERY_KEY }))
      .catch(() => {
        // Usage only orders the list; never interrupt showing the card for it.
      })
  }, [cardId, queryClient])
}
