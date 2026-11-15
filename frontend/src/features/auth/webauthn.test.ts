import { WebAuthnError, startAuthentication, startRegistration } from '@simplewebauthn/browser'
import { describe, expect, it, vi } from 'vitest'
import { createPasskey, passkeyErrorMessage, requestPasskey } from './webauthn'

vi.mock('@simplewebauthn/browser', async (importOriginal) => ({
  ...(await importOriginal<typeof import('@simplewebauthn/browser')>()),
  startRegistration: vi.fn(),
  startAuthentication: vi.fn(),
}))

describe('createPasskey / requestPasskey', () => {
  it('unwraps the publicKey options for registration', async () => {
    vi.mocked(startRegistration).mockResolvedValue({ id: 'cred' } as never)

    await expect(createPasskey({ publicKey: { challenge: 'abc' } })).resolves.toEqual({ id: 'cred' })
    expect(startRegistration).toHaveBeenCalledWith({ optionsJSON: { challenge: 'abc' } })
  })

  it('unwraps the publicKey options for sign-in', async () => {
    vi.mocked(startAuthentication).mockResolvedValue({ id: 'cred' } as never)

    await requestPasskey({ publicKey: { challenge: 'xyz' }, mediation: null })
    expect(startAuthentication).toHaveBeenCalledWith({ optionsJSON: { challenge: 'xyz' } })
  })

  it('rejects malformed server options before calling the browser', () => {
    expect(() => createPasskey({ nope: true })).toThrow()
    expect(startRegistration).not.toHaveBeenCalled()
  })
})

describe('passkeyErrorMessage', () => {
  const named = (name: string) => Object.assign(new Error(name), { name })
  const webauthn = (code: ConstructorParameters<typeof WebAuthnError>[0]['code']) =>
    new WebAuthnError({ message: code, code, cause: new Error(code) })

  it('explains cancellations', () => {
    expect(passkeyErrorMessage(named('NotAllowedError'))).toMatch(/cancelled/)
    expect(passkeyErrorMessage(webauthn('ERROR_CEREMONY_ABORTED'))).toMatch(/cancelled/)
  })

  it('explains an already registered device', () => {
    expect(passkeyErrorMessage(webauthn('ERROR_AUTHENTICATOR_PREVIOUSLY_REGISTERED'))).toMatch(/already/)
  })

  it('explains authenticators without passkey storage', () => {
    expect(
      passkeyErrorMessage(webauthn('ERROR_AUTHENTICATOR_MISSING_DISCOVERABLE_CREDENTIAL_SUPPORT')),
    ).toMatch(/cannot store/)
  })

  it('has a generic message for other WebAuthn failures', () => {
    expect(passkeyErrorMessage(webauthn('ERROR_INVALID_DOMAIN'))).toMatch(/could not complete/)
  })

  it('ignores unrelated errors', () => {
    expect(passkeyErrorMessage(new Error('boom'))).toBeNull()
    expect(passkeyErrorMessage('nope')).toBeNull()
  })
})
