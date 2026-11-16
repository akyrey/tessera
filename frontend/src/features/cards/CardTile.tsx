import { useState } from 'react'
import { Link } from 'react-router'
import type { CardDto } from '../../api/generated/CardDto'
import { cardColor, textOn } from './cardColors'
import { expiryLabel, expiryState } from './expiry'

interface CardTileProps {
  readonly card: CardDto
  /** Signed-in user, to tell own cards from the household's. */
  readonly userId: string | undefined
}

export function CardTile({ card, userId }: CardTileProps) {
  const background = cardColor(card)
  const [today] = useState(() => new Date())
  const expiry = expiryState(card.expiresOn, today)
  return (
    <Link
      to={`/cards/${card.id}`}
      className={`flex aspect-[1.586] flex-col ${expiry === 'expired' ? 'opacity-50 grayscale' : ''} justify-between rounded-2xl p-3 shadow-sm ring-1 ring-black/5 transition-transform active:scale-95`}
      style={{ backgroundColor: background, color: textOn(background) }}
    >
      <span className="flex items-start justify-between gap-1">
        <span className="line-clamp-2 text-lg leading-tight font-semibold break-words">{card.storeName}</span>
        {card.isFavorite && (
          <span aria-label="Favourite" className="shrink-0">
            ★
          </span>
        )}
      </span>
      <span className="flex items-end justify-between gap-1 text-sm opacity-80">
        <span className="truncate">
          {card.expiresOn && expiry !== 'valid' ? expiryLabel(card.expiresOn, today) : card.memberName}
        </span>
        {card.shared && (
          <span className="shrink-0 rounded-full bg-black/15 px-2 text-xs">
            {card.ownerId === userId ? 'Shared' : card.ownerName}
          </span>
        )}
      </span>
    </Link>
  )
}
