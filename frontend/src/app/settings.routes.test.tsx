import { screen, within } from '@testing-library/react'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import * as accounts from '../api/accounts'
import { ApiError } from '../api/client'
import * as meta from '../api/meta'
import * as webauthn from '../features/auth/webauthn'
import { ceremony, member, owner, passkey } from '../test/fixtures'
import { renderRoute } from '../test/renderRoute'

vi.mock('../api/accounts')
vi.mock('../api/meta')
vi.mock('../features/auth/webauthn', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../features/auth/webauthn')>()),
  supportsPasskeys: vi.fn(() => true),
  createPasskey: vi.fn(),
}))

const INVITE_URL = 'https://tessera.example.com/join#secret'

beforeEach(() => {
  vi.mocked(meta.fetchAppInfo).mockResolvedValue({ name: 'tessera', version: '0.1.0' })
  vi.mocked(webauthn.supportsPasskeys).mockReturnValue(true)
  vi.mocked(accounts.fetchMe).mockResolvedValue(owner)
  vi.mocked(accounts.listPasskeys).mockResolvedValue([passkey('p-1', 'iPhone')])
})

function section(name: string) {
  return within(screen.getByRole('region', { name }))
}

describe('household', () => {
  it('lists members with their roles', async () => {
    renderRoute('/settings')

    const household = within(await screen.findByRole('region', { name: 'Home' }))
    expect(household.getByText('Alice')).toBeInTheDocument()
    expect(household.getByText('Bea')).toBeInTheDocument()
    expect(household.getByText('Owner')).toBeInTheDocument()
    expect(household.getByText('Member')).toBeInTheDocument()
  })

  it('lets the owner create and copy an invite link', async () => {
    vi.mocked(accounts.createHouseholdInvite).mockResolvedValue({ url: INVITE_URL, expiresAt: 1_800_600_000 })
    Object.defineProperty(navigator, 'share', { value: undefined, configurable: true })

    const { user } = renderRoute('/settings')
    // user-event installs its own clipboard stub during setup; spy on that one.
    const writeText = vi.spyOn(navigator.clipboard, 'writeText')
    await user.click(await screen.findByRole('button', { name: 'Invite someone' }))

    expect(await screen.findByLabelText('Invite link')).toHaveValue(INVITE_URL)
    await user.click(screen.getByRole('button', { name: 'Share link' }))
    expect(writeText).toHaveBeenCalledWith(INVITE_URL)
    expect(await screen.findByRole('button', { name: 'Copied!' })).toBeInTheDocument()
  })

  it('uses the native share sheet when available', async () => {
    vi.mocked(accounts.createHouseholdInvite).mockResolvedValue({ url: INVITE_URL, expiresAt: 1_800_600_000 })
    const share = vi.fn().mockResolvedValue(undefined)
    Object.defineProperty(navigator, 'share', { value: share, configurable: true })

    const { user } = renderRoute('/settings')
    await user.click(await screen.findByRole('button', { name: 'Invite someone' }))
    await user.click(await screen.findByRole('button', { name: 'Share link' }))

    expect(share).toHaveBeenCalledWith({ title: 'Join my Tessera household', url: INVITE_URL })
  })

  it('shows invite failures', async () => {
    vi.mocked(accounts.createHouseholdInvite).mockRejectedValue(
      new ApiError(403, 'forbidden', 'you are not allowed to do this'),
    )

    const { user } = renderRoute('/settings')
    await user.click(await screen.findByRole('button', { name: 'Invite someone' }))

    expect(await screen.findByRole('alert')).toHaveTextContent('You are not allowed to do this')
  })

  it('hides invites from members', async () => {
    vi.mocked(accounts.fetchMe).mockResolvedValue(member)

    renderRoute('/settings')

    await screen.findByRole('region', { name: 'Home' })
    expect(screen.queryByRole('button', { name: 'Invite someone' })).not.toBeInTheDocument()
  })
})

describe('passkeys', () => {
  it('does not allow removing the last passkey', async () => {
    renderRoute('/settings')

    const remove = await screen.findByRole('button', { name: 'Remove iPhone' })
    expect(remove).toBeDisabled()
    expect(section('Passkeys').getByText(/never used/)).toBeInTheDocument()
  })

  it('removes a passkey and refreshes the list', async () => {
    vi.mocked(accounts.listPasskeys)
      .mockResolvedValueOnce([
        passkey('p-1', 'iPhone'),
        { ...passkey('p-2', 'Mac'), lastUsedAt: 1_800_000_500 },
      ])
      .mockResolvedValue([passkey('p-1', 'iPhone')])
    vi.mocked(accounts.deletePasskey).mockResolvedValue()

    const { user } = renderRoute('/settings')
    expect(await screen.findByText(/last used/)).toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: 'Remove Mac' }))

    expect(accounts.deletePasskey).toHaveBeenCalledWith('p-2', expect.anything())
    expect(await screen.findByRole('button', { name: 'Remove iPhone' })).toBeDisabled()
    expect(screen.queryByText('Mac')).not.toBeInTheDocument()
  })

  it('adds a passkey on this device', async () => {
    vi.mocked(accounts.startAddPasskey).mockResolvedValue(ceremony)
    vi.mocked(webauthn.createPasskey).mockResolvedValue({ id: 'attestation' } as never)
    vi.mocked(accounts.finishAddPasskey).mockResolvedValue(passkey('p-2', 'Mac'))

    const { user } = renderRoute('/settings')
    await user.click(await screen.findByRole('button', { name: 'Add a passkey on this device' }))

    expect(accounts.finishAddPasskey).toHaveBeenCalledWith('c-1', { id: 'attestation' }, expect.any(String))
    expect(accounts.listPasskeys).toHaveBeenCalledTimes(2)
  })

  it('reports a device that already has a passkey', async () => {
    vi.mocked(accounts.startAddPasskey).mockResolvedValue(ceremony)
    vi.mocked(webauthn.createPasskey).mockRejectedValue(
      Object.assign(new Error('exists'), { name: 'InvalidStateError' }),
    )

    const { user } = renderRoute('/settings')
    await user.click(await screen.findByRole('button', { name: 'Add a passkey on this device' }))

    expect(await screen.findByRole('alert')).toHaveTextContent(/already has a passkey/)
  })
})

describe('sign out', () => {
  it('ends the session and returns to sign in', async () => {
    vi.mocked(accounts.logout).mockResolvedValue()

    const { router, user } = renderRoute('/settings')
    vi.mocked(accounts.fetchMe).mockResolvedValue(null)
    await user.click(await screen.findByRole('button', { name: 'Sign out' }))

    expect(await screen.findByRole('button', { name: 'Sign in with passkey' })).toBeInTheDocument()
    expect(router.state.location.pathname).toBe('/login')
  })

  it('signs out of every device', async () => {
    vi.mocked(accounts.signOutEverywhere).mockResolvedValue()

    const { router, user } = renderRoute('/settings')
    vi.mocked(accounts.fetchMe).mockResolvedValue(null)
    await user.click(await screen.findByRole('button', { name: 'Sign out of all devices' }))

    expect(await screen.findByRole('button', { name: 'Sign in with passkey' })).toBeInTheDocument()
    expect(accounts.signOutEverywhere).toHaveBeenCalled()
    expect(router.state.location.pathname).toBe('/login')
  })

  it('shows a failed sign out', async () => {
    vi.mocked(accounts.logout).mockRejectedValue(new ApiError(0, 'network_error', 'offline'))

    const { user } = renderRoute('/settings')
    await user.click(await screen.findByRole('button', { name: 'Sign out' }))

    expect(await screen.findByRole('alert')).toHaveTextContent(/connection/)
  })

  it('navigates back home', async () => {
    const { router, user } = renderRoute('/settings')
    await user.click(await screen.findByRole('link', { name: 'Done' }))
    expect(router.state.location.pathname).toBe('/')
  })
})
