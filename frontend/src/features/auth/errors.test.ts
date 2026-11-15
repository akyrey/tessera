import { describe, expect, it, vi } from 'vitest'
import { ApiError } from '../../api/client'
import { describeError } from './errors'

describe('describeError', () => {
  it('shows server messages for client errors', () => {
    expect(describeError(new ApiError(410, 'invalid_invite', 'this invite link is invalid'))).toBe(
      'This invite link is invalid',
    )
  })

  it('hides server failures behind a generic message', () => {
    expect(describeError(new ApiError(500, 'internal', 'internal error'))).toMatch(/Something went wrong/)
    expect(describeError(new ApiError(200, 'invalid_response', 'x'))).toMatch(/Something went wrong/)
  })

  it('explains connectivity problems', () => {
    expect(describeError(new ApiError(0, 'network_error', 'x'))).toMatch(/connection/)
  })

  it('maps passkey errors', () => {
    expect(describeError(Object.assign(new Error('x'), { name: 'NotAllowedError' }))).toMatch(/cancelled/)
  })

  it('logs and hides unexpected errors', () => {
    const log = vi.spyOn(console, 'error').mockImplementation(() => undefined)
    expect(describeError(new Error('boom'))).toMatch(/Something went wrong/)
    expect(log).toHaveBeenCalled()
  })
})
