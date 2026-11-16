import { useLiveQuery } from 'dexie-react-hooks'
import { type LocalCard, db } from '../../offline/db'

/** Cards stored on this device; `undefined` while IndexedDB is being read. */
export function useLocalCards(): readonly LocalCard[] | undefined {
  return useLiveQuery(() => db.cards.toArray(), [])
}

/** Whether this device has completed at least one pull from the server. */
export function useHasSynced(): boolean | undefined {
  return useLiveQuery(async () => (await db.meta.get('cursor')) !== undefined, [])
}
