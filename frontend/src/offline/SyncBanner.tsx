import { useLiveQuery } from 'dexie-react-hooks'
import { useSyncExternalStore } from 'react'
import { db, readMeta, writeMeta } from './db'
import { getSyncStatus, subscribeSyncStatus } from './syncEngine'

/** Offline state, unsent changes and changes the server refused. */
export function SyncBanner() {
  const status = useSyncExternalStore(subscribeSyncStatus, getSyncStatus)
  const pending = useLiveQuery(() => db.outbox.count(), []) ?? 0
  const problems = useLiveQuery(() => readMeta('problems'), []) ?? []

  return (
    <>
      {!status.online && (
        <p
          role="status"
          className="rounded-xl bg-amber-50 px-4 py-2 text-sm text-amber-900 ring-1 ring-amber-200"
        >
          Offline: showing the cards saved on this device.
          {pending > 0 &&
            ` ${pending} ${pending === 1 ? 'change' : 'changes'} will sync when you are back online.`}
        </p>
      )}
      {problems.length > 0 && (
        <div
          role="alert"
          className="flex items-start justify-between gap-3 rounded-xl bg-red-50 px-4 py-2 text-sm text-red-800 ring-1 ring-red-200"
        >
          <ul>
            {problems.map((problem, index) => (
              <li key={index}>{problem}</li>
            ))}
          </ul>
          <button
            type="button"
            className="font-medium underline"
            onClick={() => void writeMeta('problems', [])}
          >
            Dismiss
          </button>
        </div>
      )}
    </>
  )
}
