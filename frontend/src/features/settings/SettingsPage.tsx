import { useMutation, useQueryClient } from '@tanstack/react-query'
import { Link, useNavigate } from 'react-router'
import { logout, signOutEverywhere } from '../../api/accounts'
import { Alert } from '../../components/Alert'
import { Button } from '../../components/Button'
import { Screen } from '../../components/Screen'
import { useLiveQuery } from 'dexie-react-hooks'
import { clearLocalData, db } from '../../offline/db'
import { stopSync } from '../../offline/syncEngine'
import { describeError } from '../auth/errors'
import { useMe } from '../auth/useMe'
import { HouseholdSection } from './HouseholdSection'
import { PasskeysSection } from './PasskeysSection'

export function SettingsPage() {
  const me = useMe().data
  const queryClient = useQueryClient()
  const navigate = useNavigate()

  const unsynced = useLiveQuery(() => db.outbox.count(), []) ?? 0
  const leave = async () => {
    // Shared or lost devices must not keep a copy of the cards.
    stopSync()
    await clearLocalData()
    queryClient.clear()
    await navigate('/login', { replace: true })
  }
  const signOut = useMutation({ mutationFn: logout, onSuccess: leave })
  const signOutAll = useMutation({ mutationFn: signOutEverywhere, onSuccess: leave })
  const error = signOut.error ?? signOutAll.error

  if (!me) return null

  return (
    <Screen>
      <header className="flex items-center justify-between">
        <h1 className="text-2xl font-semibold">Settings</h1>
        <Link to="/" className="font-medium text-brand-700">
          Done
        </Link>
      </header>
      <HouseholdSection household={me.household} />
      <PasskeysSection />
      <div className="flex flex-col gap-2">
        {unsynced > 0 && (
          <p
            role="status"
            className="rounded-xl bg-amber-50 px-4 py-2 text-sm text-amber-900 ring-1 ring-amber-200"
          >
            {unsynced} {unsynced === 1 ? 'change has' : 'changes have'} not synced yet. Signing out now
            discards {unsynced === 1 ? 'it' : 'them'}; reconnect first to keep{' '}
            {unsynced === 1 ? 'it' : 'them'}.
          </p>
        )}
        <Button variant="secondary" busy={signOut.isPending} onClick={() => signOut.mutate()}>
          Sign out
        </Button>
        <Button variant="danger" busy={signOutAll.isPending} onClick={() => signOutAll.mutate()}>
          Sign out of all devices
        </Button>
      </div>
      {error && <Alert>{describeError(error)}</Alert>}
    </Screen>
  )
}
