import { describe, expect, it } from 'vitest'
import { ApiError } from '../api/client'
import { shouldRetry } from './queryClient'

describe('shouldRetry', () => {
  it('retries transient failures a limited number of times', () => {
    expect(shouldRetry(0, new ApiError(0, 'network_error', ''))).toBe(true)
    expect(shouldRetry(1, new ApiError(503, 'unavailable', ''))).toBe(true)
    expect(shouldRetry(2, new ApiError(503, 'unavailable', ''))).toBe(false)
    expect(shouldRetry(0, new Error('unknown'))).toBe(true)
  })

  it('never retries client errors', () => {
    expect(shouldRetry(0, new ApiError(401, 'unauthorized', ''))).toBe(false)
    expect(shouldRetry(0, new ApiError(410, 'invalid_invite', ''))).toBe(false)
  })
})
