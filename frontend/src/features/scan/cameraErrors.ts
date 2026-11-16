/** A user-facing explanation for a `getUserMedia` failure. */
export function cameraErrorMessage(failure: unknown): string {
  const name = failure instanceof Error ? failure.name : ''
  if (name === 'NotAllowedError' || name === 'SecurityError') {
    return 'Camera access was denied. Allow it in the browser settings, or scan from a photo instead.'
  }
  if (name === 'NotFoundError' || name === 'OverconstrainedError')
    return 'No camera found. Scan from a photo instead.'
  return 'The camera is not available here. Scan from a photo instead.'
}
