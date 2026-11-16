import { useQueryClient } from '@tanstack/react-query'
import { useEffect } from 'react'
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
  const userId = me.data?.user.id

  useEffect(() => {
    if (me.data) void rememberMe(me.data)
  }, [me.data])

  // Background sync runs while someone is signed in; a 401 re-checks the session.
  useEffect(() => {
    if (!userId) return
    return startSync(userId, () => void queryClient.invalidateQueries({ queryKey: ME_QUERY_KEY }))
  }, [userId, queryClient])

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
  return <Outlet />
}
