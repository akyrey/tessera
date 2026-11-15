import { registerSW } from 'virtual:pwa-register'

/** Registers the service worker; it updates itself when a new build is deployed. */
export function registerServiceWorker(): void {
  if (!('serviceWorker' in navigator)) return
  registerSW({
    immediate: true,
    onRegisterError(error: unknown) {
      console.error('Service worker registration failed', error)
    },
  })
}
