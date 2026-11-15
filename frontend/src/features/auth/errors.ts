import { isApiError } from '../../api/client'
import { passkeyErrorMessage } from './webauthn'

const GENERIC = 'Something went wrong. Please try again.'

/** One message for any failure in a sign-in or registration flow. */
export function describeError(error: unknown): string {
  const passkeyMessage = passkeyErrorMessage(error)
  if (passkeyMessage) return passkeyMessage
  if (isApiError(error)) {
    if (error.code === 'network_error') return 'Cannot reach the server. Check your connection.'
    if (error.status >= 500 || error.code === 'invalid_response') return GENERIC
    return capitalize(error.message)
  }
  console.error('Unexpected error', error)
  return GENERIC
}

function capitalize(text: string): string {
  return text.charAt(0).toUpperCase() + text.slice(1)
}
