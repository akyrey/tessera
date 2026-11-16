import { type QueryClient, useQuery } from '@tanstack/react-query'
import type { Me } from '../../api/generated/Me'
import { loadMe } from '../../offline/session'

export const ME_QUERY_KEY = ['me'] as const

/** Current user (the remembered one when offline); `data` is `null` when signed out. */
export function useMe() {
  return useQuery({
    queryKey: ME_QUERY_KEY,
    queryFn: ({ signal }) => loadMe(signal),
    staleTime: 60_000,
  })
}

/**
 * Records a fresh sign-in. A session check still in flight (e.g. the login
 * page's "signed out" answer) is cancelled so it cannot overwrite the new user.
 */
export async function signedIn(queryClient: QueryClient, me: Me): Promise<void> {
  await queryClient.cancelQueries({ queryKey: ME_QUERY_KEY })
  queryClient.setQueryData(ME_QUERY_KEY, me)
}
