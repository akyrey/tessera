import { describe, expect, it, vi } from 'vitest'
import { z } from 'zod'
import { ApiError, getJson, isApiError, postJson, putBlob, putJson, sendRequest } from './client'

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
      expect.objectContaining({
        method: 'GET',
        credentials: 'same-origin',
        headers: { Accept: 'application/json' },
        body: undefined,
      }),
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

    await expect(getJson('/api/version', schema)).rejects.toMatchObject({ status: 502, code: 'http_error' })
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

    await expect(getJson('/api/version', schema)).rejects.toMatchObject({ status: 0, code: 'network_error' })
  })

  it('lets aborts propagate unchanged', async () => {
    const abort = new DOMException('aborted', 'AbortError')
    mockFetch(abort)

    await expect(getJson('/api/version', schema)).rejects.toBe(abort)
  })
})

describe('postJson', () => {
  it('sends the body as JSON', async () => {
    const fetchMock = mockFetch(jsonResponse({ name: 'ok' }))

    await postJson('/api/things', schema, { a: 1 })

    expect(fetchMock).toHaveBeenCalledWith(
      '/api/things',
      expect.objectContaining({
        method: 'POST',
        body: '{"a":1}',
        headers: { Accept: 'application/json', 'Content-Type': 'application/json' },
      }),
    )
  })

  it('omits the body and content type when there is no payload', async () => {
    const fetchMock = mockFetch(jsonResponse({ name: 'ok' }))

    await postJson('/api/things', schema)

    expect(fetchMock.mock.calls[0]?.[1]).toMatchObject({
      body: undefined,
      headers: { Accept: 'application/json' },
    })
  })
})

describe('putJson', () => {
  it('sends the body as JSON with PUT', async () => {
    const fetchMock = mockFetch(jsonResponse({ name: 'ok' }))

    await expect(putJson('/api/things/1', schema, { a: 1 })).resolves.toEqual({ name: 'ok' })

    expect(fetchMock.mock.calls[0]?.[1]).toMatchObject({ method: 'PUT', body: '{"a":1}' })
  })
})

describe('putBlob', () => {
  it('sends the blob unchanged with its own content type', async () => {
    const fetchMock = mockFetch(jsonResponse({ name: 'ok' }))
    const blob = new Blob(['x'], { type: 'image/webp' })

    await putBlob('/api/things/1/image', schema, blob)

    expect(fetchMock.mock.calls[0]?.[1]).toMatchObject({
      method: 'PUT',
      body: blob,
      headers: { Accept: 'application/json', 'Content-Type': 'image/webp' },
    })
  })
})

describe('sendRequest', () => {
  it('resolves on an empty success response', async () => {
    mockFetch(new Response(null, { status: 204 }))
    await expect(sendRequest('DELETE', '/api/things/1')).resolves.toBeUndefined()
  })

  it('throws the API error on failure', async () => {
    mockFetch(jsonResponse({ error: { code: 'conflict', message: 'nope' } }, 409))
    await expect(sendRequest('DELETE', '/api/things/1')).rejects.toMatchObject({
      status: 409,
      code: 'conflict',
    })
  })
})

describe('isApiError', () => {
  it('matches API errors, optionally by status', () => {
    const error = new ApiError(401, 'unauthorized', 'sign in')
    expect(isApiError(error)).toBe(true)
    expect(isApiError(error, 401)).toBe(true)
    expect(isApiError(error, 403)).toBe(false)
    expect(isApiError(new Error('x'))).toBe(false)
  })
})
