import { useQuery } from '@tanstack/react-query'
import { fetchMe } from '../../api/accounts'

export const ME_QUERY_KEY = ['me'] as const

/** Current user; `data` is `null` when signed out. */
export function useMe() {
  return useQuery({
    queryKey: ME_QUERY_KEY,
    queryFn: ({ signal }) => fetchMe(signal),
    staleTime: 60_000,
  })
}
