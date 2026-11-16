import { useEffect } from 'react'

/**
 * Keeps the screen on while the component is mounted, so the barcode does not
 * dim at the till. The lock is dropped by the browser when the tab is hidden,
 * so it is requested again when the page becomes visible.
 */
export function useWakeLock(): void {
  useEffect(() => {
    if (!('wakeLock' in navigator)) return
    let sentinel: WakeLockSentinel | null = null
    let active = true

    const acquire = async () => {
      if (document.visibilityState !== 'visible') return
      try {
        const lock = await navigator.wakeLock.request('screen')
        if (active) sentinel = lock
        else void lock.release()
      } catch {
        // Denied (e.g. low battery mode): the card still works, the screen may just dim.
      }
    }
    const onVisibility = () => void acquire()

    void acquire()
    document.addEventListener('visibilitychange', onVisibility)
    return () => {
      active = false
      document.removeEventListener('visibilitychange', onVisibility)
      void sentinel?.release()
    }
  }, [])
}
