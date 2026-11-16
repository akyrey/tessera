import { Link } from 'react-router'
import type { CardDto } from '../../api/generated/CardDto'
import { cardColor, textOn } from './cardColors'

export function CardTile({ card }: { readonly card: CardDto }) {
  const background = cardColor(card)
  return (
    <Link
      to={`/cards/${card.id}`}
      className="flex aspect-[1.586] flex-col justify-between rounded-2xl p-3 shadow-sm ring-1 ring-black/5 transition-transform active:scale-95"
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
      {card.memberName && <span className="truncate text-sm opacity-80">{card.memberName}</span>}
    </Link>
  )
}
