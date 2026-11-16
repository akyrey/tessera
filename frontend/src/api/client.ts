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

export function isApiError(error: unknown, status?: number): error is ApiError {
  return error instanceof ApiError && (status === undefined || error.status === status)
}

const errorBodySchema = z.object({
  error: z.object({ code: z.string(), message: z.string() }),
}) satisfies z.ZodType<ErrorBody>

type Method = 'GET' | 'POST' | 'PUT' | 'DELETE'

interface RequestOptions {
  /** JSON-encoded unless it is a `Blob`, which is sent as-is with its own type. */
  readonly body?: unknown
  readonly signal?: AbortSignal
}

/** GETs JSON from the same-origin API and validates it against `schema`. */
export function getJson<T>(path: string, schema: z.ZodType<T>, signal?: AbortSignal): Promise<T> {
  return requestJson('GET', path, schema, { signal })
}

/** POSTs `body` as JSON and validates the JSON response against `schema`. */
export function postJson<T>(path: string, schema: z.ZodType<T>, body?: unknown): Promise<T> {
  return requestJson('POST', path, schema, { body })
}

/** PUTs `body` as JSON and validates the JSON response against `schema`. */
export function putJson<T>(path: string, schema: z.ZodType<T>, body: unknown): Promise<T> {
  return requestJson('PUT', path, schema, { body })
}

/** PUTs a binary body (e.g. a photo) and validates the JSON response against `schema`. */
export function putBlob<T>(path: string, schema: z.ZodType<T>, blob: Blob): Promise<T> {
  return requestJson('PUT', path, schema, { body: blob })
}

/** Sends a request whose successful response has no body (e.g. 204). */
export async function sendRequest(method: Method, path: string, body?: unknown): Promise<void> {
  const response = await send(method, path, { body })
  if (!response.ok) throw toApiError(response, await readJson(response))
}

async function requestJson<T>(
  method: Method,
  path: string,
  schema: z.ZodType<T>,
  options: RequestOptions,
): Promise<T> {
  const response = await send(method, path, options)
  const body = await readJson(response)

  if (!response.ok) throw toApiError(response, body)

  const parsed = schema.safeParse(body)
  if (!parsed.success) {
    throw new ApiError(response.status, 'invalid_response', `Unexpected response from ${path}`)
  }
  return parsed.data
}

async function send(method: Method, path: string, { body, signal }: RequestOptions): Promise<Response> {
  const headers: Record<string, string> = { Accept: 'application/json' }
  const isBlob = body instanceof Blob
  if (isBlob) headers['Content-Type'] = body.type
  else if (body !== undefined) headers['Content-Type'] = 'application/json'
  try {
    return await fetch(path, {
      method,
      credentials: 'same-origin',
      headers,
      body: isBlob || body === undefined ? body : JSON.stringify(body),
      signal,
    })
  } catch (error) {
    if (error instanceof DOMException && error.name === 'AbortError') throw error
    throw new ApiError(0, 'network_error', 'Cannot reach the server')
  }
}

function readJson(response: Response): Promise<unknown> {
  return response.json().catch(() => undefined)
}

function toApiError(response: Response, body: unknown): ApiError {
  const envelope = errorBodySchema.safeParse(body)
  if (envelope.success) {
    return new ApiError(response.status, envelope.data.error.code, envelope.data.error.message)
  }
  return new ApiError(response.status, 'http_error', `Request failed with status ${response.status}`)
}
