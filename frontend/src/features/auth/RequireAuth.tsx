import { useQueryClient } from '@tanstack/react-query'
import { useEffect, useState } from 'react'
import { Navigate, Outlet } from 'react-router'
import { rememberMe } from '../../offline/session'
import { startSync } from '../../offline/syncEngine'
import { Alert } from '../../components/Alert'
import { Button } from '../../components/Button'
import { Screen } from '../../components/Screen'
import { FullPageSpinner } from '../../components/Spinner'
import { describeError } from './errors'
import { ME_QUERY_KEY, useMe } from './useMe'

/** Renders child routes only for a signed-in user. */
export function RequireAuth() {
  const me = useMe()
  const queryClient = useQueryClient()
  // The device's data is switched to this user (wiping another account's copy)
  // before anything is shown or synced.
  const [readyUserId, setReadyUserId] = useState<string | null>(null)

  useEffect(() => {
    const user = me.data
    if (!user) return
    let active = true
    rememberMe(user)
      .catch((error: unknown) => console.error('Could not store the signed-in user on this device', error))
      .finally(() => {
        if (active) setReadyUserId(user.user.id)
      })
    return () => {
      active = false
    }
  }, [me.data])

  // Background sync runs while someone is signed in; a 401 re-checks the session.
  useEffect(() => {
    if (!readyUserId) return
    return startSync(readyUserId, () => void queryClient.invalidateQueries({ queryKey: ME_QUERY_KEY }))
  }, [readyUserId, queryClient])

  if (me.isPending) return <FullPageSpinner />
  if (me.isError) {
    return (
      <Screen centered>
        <Alert>{describeError(me.error)}</Alert>
        <Button onClick={() => void me.refetch()}>Retry</Button>
      </Screen>
    )
  }
  if (!me.data) return <Navigate to="/login" replace />
  if (readyUserId !== me.data.user.id) return <FullPageSpinner />
  return <Outlet />
}
