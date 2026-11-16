import { fetchMe } from '../api/accounts'
import { isApiError } from '../api/client'
import type { Me } from '../api/generated/Me'
import { clearLocalData, readMeta, writeMeta } from './db'

/**
 * The signed-in user. Without network, the last known user is returned so the
 * cards stay usable; a definite "signed out" wipes this device's copy.
 */
export async function loadMe(signal?: AbortSignal): Promise<Me | null> {
  try {
    const me = await fetchMe(signal)
    if (me) await rememberMe(me)
    else await clearLocalData()
    return me
  } catch (error) {
    if (isApiError(error) && error.code === 'network_error') {
      const cached = await readMeta('me')
      if (cached) return cached
    }
    throw error
  }
}

/** Stores the user for offline starts; another account on this device starts from scratch. */
export async function rememberMe(me: Me): Promise<void> {
  const previous = await readMeta('me')
  if (previous && previous.user.id !== me.user.id) await clearLocalData()
  await writeMeta('me', me)
}
