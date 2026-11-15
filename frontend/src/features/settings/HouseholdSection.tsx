import { useMutation } from '@tanstack/react-query'
import { useState } from 'react'
import { createHouseholdInvite } from '../../api/accounts'
import type { HouseholdDto } from '../../api/generated/HouseholdDto'
import { Alert } from '../../components/Alert'
import { Button } from '../../components/Button'
import { Section } from '../../components/Section'
import { describeError } from '../auth/errors'
import { formatDate } from './formatDate'

export function HouseholdSection({ household }: { readonly household: HouseholdDto }) {
  return (
    <Section title={household.name}>
      <ul className="flex flex-col divide-y divide-brand-900/10">
        {household.members.map((member) => (
          <li key={member.userId} className="flex items-center justify-between py-2">
            <span>{member.displayName}</span>
            <span className="text-sm text-brand-900/60">{member.role === 'owner' ? 'Owner' : 'Member'}</span>
          </li>
        ))}
      </ul>
      {household.role === 'owner' && <InviteButton />}
    </Section>
  )
}

function InviteButton() {
  const [copied, setCopied] = useState(false)
  const invite = useMutation({ mutationFn: createHouseholdInvite })

  const share = async (url: string) => {
    if (typeof navigator.share === 'function') {
      await navigator.share({ title: 'Join my Tessera household', url }).catch(() => undefined)
      return
    }
    await navigator.clipboard.writeText(url)
    setCopied(true)
  }

  if (!invite.data) {
    return (
      <>
        <Button variant="secondary" busy={invite.isPending} onClick={() => invite.mutate()}>
          Invite someone
        </Button>
        {invite.isError && <Alert>{describeError(invite.error)}</Alert>}
      </>
    )
  }

  return (
    <div className="flex flex-col gap-2 rounded-xl bg-brand-50 p-3">
      <p className="text-sm">
        Send this link. It works once and expires {formatDate(invite.data.expiresAt)}.
      </p>
      <input
        readOnly
        aria-label="Invite link"
        value={invite.data.url}
        onFocus={(event) => event.target.select()}
        className="rounded-lg bg-white px-3 py-2 font-mono text-xs ring-1 ring-brand-900/15"
      />
      <Button onClick={() => void share(invite.data.url)}>{copied ? 'Copied!' : 'Share link'}</Button>
    </div>
  )
}
