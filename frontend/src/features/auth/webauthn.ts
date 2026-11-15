import {
  type AuthenticationResponseJSON,
  type PublicKeyCredentialCreationOptionsJSON,
  type PublicKeyCredentialRequestOptionsJSON,
  type RegistrationResponseJSON,
  WebAuthnError,
  browserSupportsWebAuthn,
  startAuthentication,
  startRegistration,
} from '@simplewebauthn/browser'
import { z } from 'zod'

/** The server sends `{ publicKey: … }`; SimpleWebAuthn wants the inner object. */
const optionsEnvelope = z.object({ publicKey: z.record(z.string(), z.unknown()) })

export function supportsPasskeys(): boolean {
  return browserSupportsWebAuthn()
}

export function createPasskey(options: unknown): Promise<RegistrationResponseJSON> {
  const { publicKey } = optionsEnvelope.parse(options)
  return startRegistration({ optionsJSON: publicKey as unknown as PublicKeyCredentialCreationOptionsJSON })
}

export function requestPasskey(options: unknown): Promise<AuthenticationResponseJSON> {
  const { publicKey } = optionsEnvelope.parse(options)
  return startAuthentication({ optionsJSON: publicKey as unknown as PublicKeyCredentialRequestOptionsJSON })
}

/** Turns browser/authenticator failures into a message for the user. */
export function passkeyErrorMessage(error: unknown): string | null {
  const name = error instanceof Error ? error.name : ''
  const code = error instanceof WebAuthnError ? error.code : ''
  if (name === 'NotAllowedError' || name === 'AbortError' || code === 'ERROR_CEREMONY_ABORTED') {
    return 'The passkey prompt was cancelled or timed out. Please try again.'
  }
  if (name === 'InvalidStateError' || code === 'ERROR_AUTHENTICATOR_PREVIOUSLY_REGISTERED') {
    return 'This device already has a passkey for your account.'
  }
  if (code === 'ERROR_AUTHENTICATOR_MISSING_DISCOVERABLE_CREDENTIAL_SUPPORT') {
    return 'This authenticator cannot store passkeys. Use your phone or computer instead.'
  }
  if (error instanceof WebAuthnError) return 'Your device could not complete the passkey request.'
  return null
}
