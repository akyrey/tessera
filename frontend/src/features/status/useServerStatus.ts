import { useEffect, useState } from 'react'
import { ApiError } from '../../api/client'
import type { AppInfo } from '../../api/generated/AppInfo'
import { fetchAppInfo } from '../../api/meta'

export type ServerStatus =
  | { readonly state: 'loading' }
  | { readonly state: 'online'; readonly info: AppInfo }
  | { readonly state: 'offline'; readonly reason: string }

const GENERIC_FAILURE = 'Something went wrong'

/** Checks the server once on mount; the request is cancelled on unmount. */
export function useServerStatus(): ServerStatus {
  const [status, setStatus] = useState<ServerStatus>({ state: 'loading' })

  useEffect(() => {
    const controller = new AbortController()
    fetchAppInfo(controller.signal)
      .then((info) => setStatus({ state: 'online', info }))
      .catch((error: unknown) => {
        if (controller.signal.aborted) return
        const reason = error instanceof ApiError ? error.message : GENERIC_FAILURE
        if (!(error instanceof ApiError)) console.error('Unexpected status check failure', error)
        setStatus({ state: 'offline', reason })
      })
    return () => controller.abort()
  }, [])

  return status
}
