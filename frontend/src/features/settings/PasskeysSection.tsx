import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { deletePasskey, finishAddPasskey, listPasskeys, startAddPasskey } from '../../api/accounts'
import type { PasskeyDto } from '../../api/generated/PasskeyDto'
import { Alert } from '../../components/Alert'
import { Button } from '../../components/Button'
import { Section } from '../../components/Section'
import { guessDeviceName } from '../auth/deviceName'
import { describeError } from '../auth/errors'
import { createPasskey } from '../auth/webauthn'
import { formatDate } from './formatDate'

const PASSKEYS_QUERY_KEY = ['passkeys'] as const

export function PasskeysSection() {
  const queryClient = useQueryClient()
  const passkeys = useQuery({
    queryKey: PASSKEYS_QUERY_KEY,
    queryFn: ({ signal }) => listPasskeys(signal),
  })
  const refresh = () => queryClient.invalidateQueries({ queryKey: PASSKEYS_QUERY_KEY })

  const add = useMutation({
    mutationFn: async () => {
      const ceremony = await startAddPasskey()
      const credential = await createPasskey(ceremony.options)
      return finishAddPasskey(ceremony.ceremonyId, credential, guessDeviceName())
    },
    onSuccess: refresh,
  })
  const remove = useMutation({ mutationFn: deletePasskey, onSuccess: refresh })

  const items = passkeys.data ?? []
  const error = passkeys.error ?? add.error ?? remove.error

  return (
    <Section title="Passkeys">
      <p className="text-sm text-brand-900/60">
        Add a passkey on each device you use, so losing one phone never locks you out.
      </p>
      {passkeys.isPending ? (
        <p className="text-sm">Loading…</p>
      ) : (
        <ul className="flex flex-col divide-y divide-brand-900/10">
          {items.map((passkey) => (
            <PasskeyRow
              key={passkey.id}
              passkey={passkey}
              canRemove={items.length > 1}
              removing={remove.isPending && remove.variables === passkey.id}
              onRemove={() => remove.mutate(passkey.id)}
            />
          ))}
        </ul>
      )}
      <Button variant="secondary" busy={add.isPending} onClick={() => add.mutate()}>
        Add a passkey on this device
      </Button>
      {error && <Alert>{describeError(error)}</Alert>}
    </Section>
  )
}

interface PasskeyRowProps {
  readonly passkey: PasskeyDto
  readonly canRemove: boolean
  readonly removing: boolean
  readonly onRemove: () => void
}

function PasskeyRow({ passkey, canRemove, removing, onRemove }: PasskeyRowProps) {
  const lastUsed = passkey.lastUsedAt ? `last used ${formatDate(passkey.lastUsedAt)}` : 'never used'
  return (
    <li className="flex items-center justify-between gap-3 py-2">
      <div className="min-w-0">
        <p className="truncate font-medium">{passkey.name}</p>
        <p className="text-sm text-brand-900/60">
          Added {formatDate(passkey.createdAt)}, {lastUsed}
        </p>
      </div>
      <Button
        variant="danger"
        busy={removing}
        disabled={!canRemove}
        title={canRemove ? undefined : 'You cannot remove your last passkey'}
        aria-label={`Remove ${passkey.name}`}
        onClick={onRemove}
      >
        Remove
      </Button>
    </li>
  )
}
