import { Navigate, Outlet } from 'react-router'
import { Alert } from '../../components/Alert'
import { Button } from '../../components/Button'
import { Screen } from '../../components/Screen'
import { FullPageSpinner } from '../../components/Spinner'
import { describeError } from './errors'
import { useMe } from './useMe'

/** Renders child routes only for a signed-in user. */
export function RequireAuth() {
  const me = useMe()

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
