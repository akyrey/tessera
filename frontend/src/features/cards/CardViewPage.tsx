import { useEffect, useRef, useState } from 'react'
import { Link } from 'react-router'
import type { LocalCard } from '../../offline/db'
import { enqueue } from '../../offline/syncEngine'
import { useMe } from '../auth/useMe'
import { Barcode } from './Barcode'
import { PlacesSection } from './PlacesSection'
import { expiryLabel } from './expiry'
import { CardStatus } from './CardStatus'
import { cardColor, textOn } from './cardColors'
import { useCardFromRoute } from './useCardFromRoute'
import { useWakeLock } from './useWakeLock'
import { photoSrc } from './photoSrc'

export function CardViewPage() {
  const { card, loading } = useCardFromRoute()
  if (!card) return <CardStatus loading={loading} />
  return <CardView card={card} />
}

function CardView({ card }: { readonly card: LocalCard }) {
  useWakeLock()
  useRecordUse(card.id)
  const toggleFavorite = () => void enqueue(card.id, { type: 'favorite', isFavorite: !card.isFavorite })
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
              onClick={toggleFavorite}
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
        {card.shared && <SharedNote card={card} />}
        {card.expiresOn && <ExpiryNote expiresOn={card.expiresOn} />}
      </header>

      <section aria-label="Barcode" className="flex flex-col items-center gap-4 px-4 py-8 text-black">
        <Barcode format={card.barcodeFormat} value={card.cardNumber} />
        <p className="text-center font-mono text-2xl tracking-wider break-all select-all">
          {card.cardNumber}
        </p>
      </section>

      <div className="flex flex-col gap-4 px-4">
        {card.notes && <p className="whitespace-pre-line text-brand-900/80">{card.notes}</p>}
        <Photo label="Front of the card" src={photoSrc(card, 'front')} />
        <Photo label="Back of the card" src={photoSrc(card, 'back')} />
        <PlacesSection card={card} />
      </div>
    </main>
  )
}

function ExpiryNote({ expiresOn }: { readonly expiresOn: string }) {
  const [today] = useState(() => new Date())
  return <p className="text-sm font-medium">{expiryLabel(expiresOn, today)}</p>
}

function SharedNote({ card }: { readonly card: LocalCard }) {
  const me = useMe().data
  const mine = card.ownerId === me?.user.id
  return (
    <p className="text-sm opacity-80">
      {mine ? `Shared with ${me?.household.name ?? 'your household'}` : `Shared by ${card.ownerName}`}
    </p>
  )
}

function Photo({ label, src }: { readonly label: string; readonly src: string | null }) {
  if (!src) return null
  return <img src={src} alt={label} loading="lazy" className="w-full rounded-2xl ring-1 ring-black/10" />
}

/** Counts one use per card opening (also under StrictMode's double effects). */
function useRecordUse(cardId: string): void {
  const recorded = useRef<string | null>(null)
  useEffect(() => {
    if (recorded.current === cardId) return
    recorded.current = cardId
    void enqueue(cardId, { type: 'use', at: Math.floor(Date.now() / 1000) })
  }, [cardId])
}
