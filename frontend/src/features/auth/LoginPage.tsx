import { useMutation, useQueryClient } from '@tanstack/react-query'
import { Navigate, useNavigate } from 'react-router'
import { finishLogin, startLogin } from '../../api/accounts'
import { Alert } from '../../components/Alert'
import { Brand } from '../../components/Brand'
import { Button } from '../../components/Button'
import { Screen } from '../../components/Screen'
import { ServerStatusBadge } from '../status/ServerStatusBadge'
import { useServerStatus } from '../status/useServerStatus'
import { describeError } from './errors'
import { signedIn, useMe } from './useMe'
import { requestPasskey, supportsPasskeys } from './webauthn'

export function LoginPage() {
  const me = useMe()
  const status = useServerStatus()
  const queryClient = useQueryClient()
  const navigate = useNavigate()

  const signIn = useMutation({
    mutationFn: async () => {
      const ceremony = await startLogin()
      const credential = await requestPasskey(ceremony.options)
      return finishLogin(ceremony.ceremonyId, credential)
    },
    onSuccess: async (user) => {
      await signedIn(queryClient, user)
      await navigate('/', { replace: true })
    },
  })

  if (me.data) return <Navigate to="/" replace />

  return (
    <Screen centered>
      <Brand subtitle="Your loyalty cards, always at hand." />
      {supportsPasskeys() ? (
        <Button busy={signIn.isPending} onClick={() => signIn.mutate()}>
          Sign in with passkey
        </Button>
      ) : (
        <Alert>This browser does not support passkeys. Please update it or use another browser.</Alert>
      )}
      {signIn.isError && <Alert>{describeError(signIn.error)}</Alert>}
      <p className="text-center text-sm text-brand-900/60">
        New here? Ask someone in your household for an invite link.
      </p>
      <div className="flex justify-center">
        <ServerStatusBadge status={status} />
      </div>
    </Screen>
  )
}
