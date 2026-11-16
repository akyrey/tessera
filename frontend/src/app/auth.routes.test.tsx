import { screen, waitFor } from '@testing-library/react'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import * as accounts from '../api/accounts'
import * as cards from '../api/cards'
import { ApiError } from '../api/client'
import * as meta from '../api/meta'
import * as webauthn from '../features/auth/webauthn'
import { ceremony, member, owner } from '../test/fixtures'
import { renderRoute } from '../test/renderRoute'

vi.mock('../api/accounts')
vi.mock('../api/meta')
vi.mock('../api/cards')
vi.mock('../features/auth/webauthn', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../features/auth/webauthn')>()),
  supportsPasskeys: vi.fn(() => true),
  createPasskey: vi.fn(),
  requestPasskey: vi.fn(),
}))

beforeEach(() => {
  vi.mocked(meta.fetchAppInfo).mockResolvedValue({ name: 'tessera', version: '0.1.0' })
  vi.mocked(webauthn.supportsPasskeys).mockReturnValue(true)
  vi.mocked(cards.listCards).mockResolvedValue([])
})

describe('route guard', () => {
  it('sends signed-out visitors to the sign-in page', async () => {
    vi.mocked(accounts.fetchMe).mockResolvedValue(null)

    const { router } = renderRoute('/')

    expect(await screen.findByRole('button', { name: 'Sign in with passkey' })).toBeInTheDocument()
    expect(router.state.location.pathname).toBe('/login')
  })

  it('shows the home page to signed-in users', async () => {
    vi.mocked(accounts.fetchMe).mockResolvedValue(owner)

    renderRoute('/')

    expect(await screen.findByRole('heading', { name: 'Cards' })).toBeInTheDocument()
  })

  it('offers a retry when the session check fails', async () => {
    vi.mocked(accounts.fetchMe)
      .mockRejectedValueOnce(new ApiError(0, 'network_error', 'offline'))
      .mockResolvedValue(owner)

    const { user } = renderRoute('/')

    expect(await screen.findByRole('alert')).toHaveTextContent(/connection/)
    await user.click(screen.getByRole('button', { name: 'Retry' }))
    expect(await screen.findByRole('heading', { name: 'Cards' })).toBeInTheDocument()
  })

  it('redirects unknown paths home', async () => {
    vi.mocked(accounts.fetchMe).mockResolvedValue(owner)

    const { router } = renderRoute('/nope')

    await waitFor(() => expect(router.state.location.pathname).toBe('/'))
  })
})

describe('sign in', () => {
  beforeEach(() => vi.mocked(accounts.fetchMe).mockResolvedValue(null))

  it('signs in with a passkey and lands on the home page', async () => {
    vi.mocked(accounts.startLogin).mockResolvedValue(ceremony)
    vi.mocked(webauthn.requestPasskey).mockResolvedValue({ id: 'assertion' } as never)
    vi.mocked(accounts.finishLogin).mockResolvedValue(owner)

    const { router, user } = renderRoute('/login')
    await user.click(await screen.findByRole('button', { name: 'Sign in with passkey' }))

    expect(await screen.findByRole('heading', { name: 'Cards' })).toBeInTheDocument()
    expect(webauthn.requestPasskey).toHaveBeenCalledWith(ceremony.options)
    expect(accounts.finishLogin).toHaveBeenCalledWith('c-1', { id: 'assertion' })
    expect(router.state.location.pathname).toBe('/')
  })

  it('explains a cancelled passkey prompt', async () => {
    vi.mocked(accounts.startLogin).mockResolvedValue(ceremony)
    vi.mocked(webauthn.requestPasskey).mockRejectedValue(
      Object.assign(new Error('cancelled'), { name: 'NotAllowedError' }),
    )

    const { user } = renderRoute('/login')
    await user.click(await screen.findByRole('button', { name: 'Sign in with passkey' }))

    expect(await screen.findByRole('alert')).toHaveTextContent(/cancelled/)
    expect(accounts.finishLogin).not.toHaveBeenCalled()
  })

  it('warns when the browser has no passkey support', async () => {
    vi.mocked(webauthn.supportsPasskeys).mockReturnValue(false)

    renderRoute('/login')

    expect(await screen.findByRole('alert')).toHaveTextContent(/does not support passkeys/)
    expect(screen.queryByRole('button', { name: 'Sign in with passkey' })).not.toBeInTheDocument()
  })

  it('skips the sign-in page when already signed in', async () => {
    vi.mocked(accounts.fetchMe).mockResolvedValue(owner)

    const { router } = renderRoute('/login')

    await waitFor(() => expect(router.state.location.pathname).toBe('/'))
  })
})

describe('invite links', () => {
  beforeEach(() => vi.mocked(accounts.fetchMe).mockResolvedValue(null))

  it('rejects a link without a token', async () => {
    renderRoute('/join')

    expect(await screen.findByRole('alert')).toHaveTextContent(/missing its invite code/)
    expect(accounts.previewInvite).not.toHaveBeenCalled()
  })

  it('explains an expired or used link', async () => {
    vi.mocked(accounts.previewInvite).mockRejectedValue(
      new ApiError(410, 'invalid_invite', 'this invite link is invalid, expired or already used'),
    )

    renderRoute('/join#tok')

    expect(await screen.findByRole('alert')).toHaveTextContent(
      'This invite link is invalid, expired or already used',
    )
  })

  it('joins a household: name, passkey, then home', async () => {
    vi.mocked(accounts.previewInvite).mockResolvedValue({
      kind: 'household',
      householdName: 'Home',
      displayName: null,
    })
    vi.mocked(accounts.startRegistration).mockResolvedValue(ceremony)
    vi.mocked(webauthn.createPasskey).mockResolvedValue({ id: 'attestation' } as never)
    vi.mocked(accounts.finishRegistration).mockResolvedValue(member)

    const { user } = renderRoute('/join#tok')
    expect(await screen.findByText(/invited to join "Home"/)).toBeInTheDocument()
    const submit = screen.getByRole('button', { name: 'Create passkey' })
    expect(submit).toBeDisabled()

    await user.type(screen.getByLabelText('Your name'), '  Bea ')
    await user.click(submit)

    expect(await screen.findByRole('heading', { name: 'Cards' })).toBeInTheDocument()
    expect(accounts.previewInvite).toHaveBeenCalledWith('tok')
    expect(window.location.hash).toBe('')
    expect(accounts.startRegistration).toHaveBeenCalledWith({ token: 'tok', displayName: 'Bea' })
    expect(accounts.finishRegistration).toHaveBeenCalledWith('c-1', { id: 'attestation' }, expect.any(String))
  })

  it('sets up the first administrator', async () => {
    vi.mocked(accounts.previewInvite).mockResolvedValue({
      kind: 'bootstrap',
      householdName: null,
      displayName: null,
    })

    renderRoute('/join#tok')

    expect(await screen.findByText(/You will be the administrator/)).toBeInTheDocument()
    expect(screen.getByLabelText('Your name')).toBeInTheDocument()
  })

  it('recovers an account without asking for a name', async () => {
    vi.mocked(accounts.previewInvite).mockResolvedValue({
      kind: 'recovery',
      householdName: null,
      displayName: 'Alice',
    })
    vi.mocked(accounts.startRegistration).mockResolvedValue(ceremony)
    vi.mocked(webauthn.createPasskey).mockResolvedValue({ id: 'attestation' } as never)
    vi.mocked(accounts.finishRegistration).mockResolvedValue(owner)

    const { user } = renderRoute('/join#tok')
    expect(await screen.findByText(/new passkey for Alice/)).toBeInTheDocument()
    expect(screen.queryByLabelText('Your name')).not.toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: 'Create passkey' }))

    expect(await screen.findByRole('heading', { name: 'Cards' })).toBeInTheDocument()
    expect(accounts.startRegistration).toHaveBeenCalledWith({ token: 'tok', displayName: undefined })
  })

  it('shows registration failures', async () => {
    vi.mocked(accounts.previewInvite).mockResolvedValue({
      kind: 'recovery',
      householdName: null,
      displayName: 'Alice',
    })
    vi.mocked(accounts.startRegistration).mockRejectedValue(
      new ApiError(410, 'invalid_invite', 'link already used'),
    )

    const { user } = renderRoute('/join#tok')
    await user.click(await screen.findByRole('button', { name: 'Create passkey' }))

    expect(await screen.findByRole('alert')).toHaveTextContent('Link already used')
  })
})
