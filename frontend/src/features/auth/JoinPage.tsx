import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { type FormEvent, useEffect, useState } from 'react'
import { useLocation, useNavigate } from 'react-router'
import { finishRegistration, previewInvite, startRegistration } from '../../api/accounts'
import type { InvitePreview } from '../../api/generated/InvitePreview'
import { Alert } from '../../components/Alert'
import { Brand } from '../../components/Brand'
import { Button } from '../../components/Button'
import { Screen } from '../../components/Screen'
import { FullPageSpinner } from '../../components/Spinner'
import { guessDeviceName } from './deviceName'
import { describeError } from './errors'
import { signedIn } from './useMe'
import { createPasskey } from './webauthn'

const MAX_NAME_LENGTH = 64

/** Landing page for invite links: `/join#<token>`. The token never leaves the fragment. */
export function JoinPage() {
  const hash = useLocation().hash
  // Read once, then drop the secret from the address bar and session history.
  const [token] = useState(() => hash.replace(/^#/, ''))
  useEffect(() => {
    if (window.location.hash) window.history.replaceState(window.history.state, '', window.location.pathname)
  }, [])
  const invite = useQuery({
    queryKey: ['invite', token],
    queryFn: () => previewInvite(token),
    enabled: token.length > 0,
    retry: false,
    staleTime: Infinity,
  })

  if (!token) return <InvalidInvite message="This link is missing its invite code." />
  if (invite.isPending) return <FullPageSpinner />
  if (invite.isError) return <InvalidInvite message={describeError(invite.error)} />
  return <RegistrationForm token={token} invite={invite.data} />
}

function InvalidInvite({ message }: { readonly message: string }) {
  return (
    <Screen centered>
      <Brand />
      <Alert>{message}</Alert>
      <p className="text-center text-sm text-brand-900/60">Ask for a new invite link, or sign in instead.</p>
      <a href="/login" className="text-center font-medium text-brand-700 underline">
        Go to sign in
      </a>
    </Screen>
  )
}

function RegistrationForm({ token, invite }: { readonly token: string; readonly invite: InvitePreview }) {
  const [displayName, setDisplayName] = useState('')
  const queryClient = useQueryClient()
  const navigate = useNavigate()
  const needsName = invite.kind !== 'recovery'

  const register = useMutation({
    mutationFn: async () => {
      const ceremony = await startRegistration({
        token,
        displayName: needsName ? displayName.trim() : undefined,
      })
      const credential = await createPasskey(ceremony.options)
      return finishRegistration(ceremony.ceremonyId, credential, guessDeviceName())
    },
    onSuccess: async (me) => {
      await signedIn(queryClient, me)
      await navigate('/', { replace: true })
    },
  })

  const onSubmit = (event: FormEvent) => {
    event.preventDefault()
    register.mutate()
  }

  return (
    <Screen centered>
      <Brand subtitle={describeInvite(invite)} />
      <form onSubmit={onSubmit} className="flex flex-col gap-4">
        {needsName && (
          <label className="flex flex-col gap-1.5">
            <span className="text-sm font-medium">Your name</span>
            <input
              value={displayName}
              onChange={(event) => setDisplayName(event.target.value)}
              required
              maxLength={MAX_NAME_LENGTH}
              autoComplete="given-name"
              className="min-h-11 rounded-xl bg-white px-3 ring-1 ring-brand-900/20 focus:ring-2 focus:ring-brand-700 focus:outline-none"
            />
          </label>
        )}
        <Button type="submit" busy={register.isPending} disabled={needsName && !displayName.trim()}>
          Create passkey
        </Button>
      </form>
      {register.isError && <Alert>{describeError(register.error)}</Alert>}
      <p className="text-center text-sm text-brand-900/60">
        Your phone will ask for Face ID, fingerprint or your screen lock. No password needed.
      </p>
    </Screen>
  )
}

function describeInvite(invite: InvitePreview): string {
  switch (invite.kind) {
    case 'bootstrap':
      return 'Set up Tessera. You will be the administrator.'
    case 'household':
      return `You have been invited to join "${invite.householdName ?? 'a household'}".`
    case 'recovery':
      return `Add a new passkey for ${invite.displayName ?? 'your account'}.`
  }
}
