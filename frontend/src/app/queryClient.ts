import { QueryClient } from '@tanstack/react-query'
import { isApiError } from '../api/client'

const MAX_RETRIES = 2

/** Retries only failures that may be transient (network, 5xx); 4xx answers are final. */
export function shouldRetry(failureCount: number, error: unknown): boolean {
  if (failureCount >= MAX_RETRIES) return false
  return !isApiError(error) || error.status === 0 || error.status >= 500
}

export function createQueryClient(): QueryClient {
  return new QueryClient({
    defaultOptions: {
      queries: { retry: shouldRetry, refetchOnWindowFocus: false },
      mutations: { retry: false },
    },
  })
}
