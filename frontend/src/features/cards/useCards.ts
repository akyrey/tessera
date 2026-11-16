import { type QueryClient, useQuery } from '@tanstack/react-query'
import { listCards } from '../../api/cards'
import type { CardDto } from '../../api/generated/CardDto'

export const CARDS_QUERY_KEY = ['cards'] as const

export function useCards() {
  return useQuery({
    queryKey: CARDS_QUERY_KEY,
    queryFn: ({ signal }) => listCards(signal),
    staleTime: 30_000,
  })
}

/** Puts a saved card into the cached list without refetching it. */
export function storeCard(queryClient: QueryClient, card: CardDto): void {
  queryClient.setQueryData<CardDto[]>(CARDS_QUERY_KEY, (cards = []) => {
    const others = cards.filter((existing) => existing.id !== card.id)
    return [...others, card]
  })
}

export function forgetCard(queryClient: QueryClient, id: string): void {
  queryClient.setQueryData<CardDto[]>(CARDS_QUERY_KEY, (cards = []) => cards.filter((card) => card.id !== id))
}
