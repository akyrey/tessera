import { z } from 'zod'
import type { ErrorBody } from './generated/ErrorBody'

/** Error codes produced on the client side, in addition to the server's codes. */
export type ClientErrorCode = 'network_error' | 'http_error' | 'invalid_response'

export class ApiError extends Error {
  readonly status: number
  readonly code: string

  constructor(status: number, code: string, message: string) {
    super(message)
    this.name = 'ApiError'
    this.status = status
    this.code = code
  }
}

const errorBodySchema = z.object({
  error: z.object({ code: z.string(), message: z.string() }),
}) satisfies z.ZodType<ErrorBody>

/** GETs JSON from the same-origin API and validates it against `schema`. */
export async function getJson<T>(path: string, schema: z.ZodType<T>, signal?: AbortSignal): Promise<T> {
  const response = await send(path, signal)
  const body: unknown = await response.json().catch(() => undefined)

  if (!response.ok) throw toApiError(response, body)

  const parsed = schema.safeParse(body)
  if (!parsed.success) {
    throw new ApiError(response.status, 'invalid_response', `Unexpected response from ${path}`)
  }
  return parsed.data
}

async function send(path: string, signal?: AbortSignal): Promise<Response> {
  try {
    return await fetch(path, {
      credentials: 'same-origin',
      headers: { Accept: 'application/json' },
      signal,
    })
  } catch (error) {
    if (error instanceof DOMException && error.name === 'AbortError') throw error
    throw new ApiError(0, 'network_error', 'Cannot reach the server')
  }
}

function toApiError(response: Response, body: unknown): ApiError {
  const envelope = errorBodySchema.safeParse(body)
  if (envelope.success) {
    return new ApiError(response.status, envelope.data.error.code, envelope.data.error.message)
  }
  return new ApiError(response.status, 'http_error', `Request failed with status ${response.status}`)
}
