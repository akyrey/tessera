import { fetchMe } from '../api/accounts'
import { isApiError } from '../api/client'
import type { Me } from '../api/generated/Me'
import { clearLocalData, db, readMeta, writeMeta } from './db'

/**
 * The signed-in user. Without network, the last known user is returned so the
 * cards stay usable; a definite "signed out" wipes this device's copy (unless
 * unsent changes are waiting). A cancelled check (superseded by a fresh
 * sign-in) changes nothing.
 */
export async function loadMe(signal?: AbortSignal): Promise<Me | null> {
  try {
    const me = await fetchMe(signal)
    if (me) await rememberMe(me)
    else if (!signal?.aborted) await forgetSignedOutUser()
    return me
  } catch (error) {
    // Unreachable or failing server (offline, proxy error, restart): keep the wallet usable.
    if (isApiError(error) && (error.code === 'network_error' || error.status >= 500)) {
      const cached = await readMeta('me')
      if (cached) return cached
    }
    throw error
  }
}

/**
 * The session ended (expired or signed out elsewhere). Unsent changes are kept
 * for when the same person signs in again; another account wipes them then.
 */
async function forgetSignedOutUser(): Promise<void> {
  if ((await db.outbox.count()) === 0) await clearLocalData()
}

/** Stores the user for offline starts; another (or an unknown) account's data is wiped first. */
export async function rememberMe(me: Me): Promise<void> {
  const previous = await readMeta('me')
  // Data whose owner is unknown (or someone else) is never shown to this user.
  if (previous?.user.id !== me.user.id) await clearLocalData()
  await writeMeta('me', me)
}
