import { describe, expect, it, vi } from 'vitest'
import * as api from './accounts'
import { deletePasskey, fetchMe, finishLogin, previewInvite } from './accounts'
import { ceremony, owner, passkey } from '../test/fixtures'

function respond(body: unknown, status = 200) {
  const fetchMock = vi.fn<typeof fetch>().mockResolvedValue(
    new Response(body === null ? null : JSON.stringify(body), {
      status,
      headers: { 'Content-Type': 'application/json' },
    }),
  )
  vi.stubGlobal('fetch', fetchMock)
  return fetchMock
}

describe('fetchMe', () => {
  it('returns the signed-in user', async () => {
    respond(owner)
    await expect(fetchMe()).resolves.toEqual(owner)
  })

  it('returns null when signed out', async () => {
    respond({ error: { code: 'unauthorized', message: 'sign in required' } }, 401)
    await expect(fetchMe()).resolves.toBeNull()
  })

  it('propagates other failures', async () => {
    respond({ error: { code: 'internal', message: 'internal error' } }, 500)
    await expect(fetchMe()).rejects.toMatchObject({ status: 500 })
  })
})

describe('account requests', () => {
  it('sends the invite token in the body, never in the URL', async () => {
    const fetchMock = respond({ kind: 'bootstrap', householdName: null, displayName: null })

    await previewInvite('secret-token')

    expect(fetchMock).toHaveBeenCalledWith(
      '/api/auth/invite',
      expect.objectContaining({ body: '{"token":"secret-token"}' }),
    )
  })

  it('posts the ceremony id and credential to finish login', async () => {
    const fetchMock = respond(owner)

    await finishLogin('c-1', { id: 'cred' })

    expect(fetchMock.mock.calls[0]?.[1]?.body).toBe('{"ceremonyId":"c-1","credential":{"id":"cred"}}')
  })

  it('encodes passkey ids in the delete URL', async () => {
    const fetchMock = respond(null, 204)

    await deletePasskey('a/b')

    expect(fetchMock.mock.calls[0]?.[0]).toBe('/api/me/passkeys/a%2Fb')
  })
})

describe('endpoint map', () => {
  it.each([
    [
      'startRegistration',
      () => api.startRegistration({ token: 't' }),
      'POST',
      '/api/auth/register/start',
      ceremony,
    ],
    [
      'finishRegistration',
      () => api.finishRegistration('c', {}, 'iPhone'),
      'POST',
      '/api/auth/register/finish',
      owner,
    ],
    ['startLogin', () => api.startLogin(), 'POST', '/api/auth/login/start', ceremony],
    ['signOutEverywhere', () => api.signOutEverywhere(), 'DELETE', '/api/me/sessions', null],
    ['logout', () => api.logout(), 'POST', '/api/auth/logout', null],
    ['listPasskeys', () => api.listPasskeys(), 'GET', '/api/me/passkeys', [passkey('p', 'Mac')]],
    ['startAddPasskey', () => api.startAddPasskey(), 'POST', '/api/me/passkeys/start', ceremony],
    [
      'finishAddPasskey',
      () => api.finishAddPasskey('c', {}, 'Mac'),
      'POST',
      '/api/me/passkeys/finish',
      passkey('p', 'Mac'),
    ],
    [
      'createHouseholdInvite',
      () => api.createHouseholdInvite(),
      'POST',
      '/api/household/invites',
      { url: 'u', expiresAt: 1 },
    ],
  ] as const)('%s → %s %s', async (_name, call, method, path, body) => {
    const fetchMock = respond(body, body === null ? 204 : 200)

    await call()

    expect(fetchMock).toHaveBeenCalledWith(path, expect.objectContaining({ method }))
  })
})
