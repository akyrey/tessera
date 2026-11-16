import { useParams } from 'react-router'
import type { LocalCard } from '../../offline/db'
import { useLocalCards } from './useCards'

export interface RouteCard {
  readonly id: string
  readonly card: LocalCard | undefined
  readonly loading: boolean
}

/** The card named by the `:id` route parameter, from the device's store. */
export function useCardFromRoute(): RouteCard {
  const { id = '' } = useParams()
  const cards = useLocalCards()
  return { id, card: cards?.find((card) => card.id === id), loading: cards === undefined }
}
