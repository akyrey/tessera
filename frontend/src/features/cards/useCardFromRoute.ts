import { useParams } from 'react-router'
import type { CardDto } from '../../api/generated/CardDto'
import { useCards } from './useCards'

/** The card named by the `:id` route parameter, from the cached list. */
export function useCardFromRoute(): {
  readonly card: CardDto | undefined
  readonly query: ReturnType<typeof useCards>
} {
  const { id } = useParams()
  const query = useCards()
  return { card: query.data?.find((card) => card.id === id), query }
}
