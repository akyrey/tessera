import { describe, expect, it, vi } from 'vitest'
import { z } from 'zod'
import { ApiError, getJson } from './client'

const schema = z.object({ name: z.string() })

function mockFetch(outcome: Response | Error | DOMException) {
  const fetchMock =
    outcome instanceof Response
      ? vi.fn<typeof fetch>().mockResolvedValue(outcome)
      : vi.fn<typeof fetch>().mockRejectedValue(outcome)
  vi.stubGlobal('fetch', fetchMock)
  return fetchMock
}

function jsonResponse(body: unknown, status = 200) {
  return new Response(JSON.stringify(body), {
    status,
    headers: { 'Content-Type': 'application/json' },
  })
}

describe('getJson', () => {
  it('returns the parsed body when it matches the schema', async () => {
    const fetchMock = mockFetch(jsonResponse({ name: 'tessera' }))

    const result = await getJson('/api/version', schema)

    expect(result).toEqual({ name: 'tessera' })
    expect(fetchMock).toHaveBeenCalledWith(
      '/api/version',
      expect.objectContaining({ credentials: 'same-origin', headers: { Accept: 'application/json' } }),
    )
  })

  it('throws the server error code and message on a non-2xx response', async () => {
    mockFetch(jsonResponse({ error: { code: 'not_found', message: 'resource not found' } }, 404))

    const error = await getJson('/api/nope', schema).catch((e: unknown) => e)

    expect(error).toBeInstanceOf(ApiError)
    expect(error).toMatchObject({ status: 404, code: 'not_found', message: 'resource not found' })
  })

  it('falls back to a generic error when the error body is not an envelope', async () => {
    mockFetch(new Response('Bad gateway', { status: 502 }))

    await expect(getJson('/api/version', schema)).rejects.toMatchObject({
      status: 502,
      code: 'http_error',
    })
  })

  it('rejects a successful response whose body does not match the schema', async () => {
    mockFetch(jsonResponse({ name: 42 }))

    await expect(getJson('/api/version', schema)).rejects.toMatchObject({
      status: 200,
      code: 'invalid_response',
    })
  })

  it('reports network failures as offline', async () => {
    mockFetch(new TypeError('Failed to fetch'))

    await expect(getJson('/api/version', schema)).rejects.toMatchObject({
      status: 0,
      code: 'network_error',
    })
  })

  it('lets aborts propagate unchanged', async () => {
    const abort = new DOMException('aborted', 'AbortError')
    mockFetch(abort)

    await expect(getJson('/api/version', schema)).rejects.toBe(abort)
  })
})
