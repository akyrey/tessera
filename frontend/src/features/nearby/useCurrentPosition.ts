import { useCallback, useEffect, useState } from 'react'
import type { LatLon } from './nearby'

export type PositionState =
  | { readonly status: 'idle' }
  | { readonly status: 'locating' }
  | { readonly status: 'ready'; readonly position: LatLon }
  | { readonly status: 'denied' }
  | { readonly status: 'unavailable' }

/** A recent fix is good enough: we only need to know which shop we are in. */
const MAX_AGE_MS = 60_000
const TIMEOUT_MS = 15_000

/**
 * One location fix when the app opens. It is taken automatically only if the
 * user already allowed it; otherwise `locate` asks (from a button tap).
 */
export function useCurrentPosition(): { readonly state: PositionState; readonly locate: () => void } {
  const [state, setState] = useState<PositionState>(() =>
    'geolocation' in navigator ? { status: 'idle' } : { status: 'unavailable' },
  )

  const locate = useCallback(() => {
    if (!('geolocation' in navigator)) return
    setState({ status: 'locating' })
    navigator.geolocation.getCurrentPosition(
      ({ coords }) =>
        setState({ status: 'ready', position: { lat: coords.latitude, lon: coords.longitude } }),
      (error) => setState({ status: error.code === error.PERMISSION_DENIED ? 'denied' : 'unavailable' }),
      { enableHighAccuracy: true, maximumAge: MAX_AGE_MS, timeout: TIMEOUT_MS },
    )
  }, [])

  useEffect(() => {
    let active = true
    navigator.permissions
      ?.query({ name: 'geolocation' })
      .then((permission) => {
        if (!active) return
        if (permission.state === 'granted') locate()
        else if (permission.state === 'denied') setState({ status: 'denied' })
      })
      .catch(() => undefined)
    return () => {
      active = false
    }
  }, [locate])

  return { state, locate }
}
