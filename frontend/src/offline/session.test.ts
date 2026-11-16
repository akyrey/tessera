import { describe, expect, it, vi } from 'vitest'
import * as accounts from '../api/accounts'
import { ApiError } from '../api/client'
import { card, member, owner } from '../test/fixtures'
import { db, readMeta } from './db'
import { loadMe, rememberMe } from './session'

vi.mock('../api/accounts')

describe('loadMe', () => {
  it('remembers the signed-in user', async () => {
    vi.mocked(accounts.fetchMe).mockResolvedValue(owner)
    await expect(loadMe()).resolves.toEqual(owner)
    expect(await readMeta('me')).toEqual(owner)
  })

  it('falls back to the remembered user when offline', async () => {
    await rememberMe(owner)
    vi.mocked(accounts.fetchMe).mockRejectedValue(new ApiError(0, 'network_error', 'offline'))
    await expect(loadMe()).resolves.toEqual(owner)
  })

  it('fails offline when nobody was ever signed in here', async () => {
    vi.mocked(accounts.fetchMe).mockRejectedValue(new ApiError(0, 'network_error', 'offline'))
    await expect(loadMe()).rejects.toMatchObject({ code: 'network_error' })
  })

  it('wipes local data when signed out', async () => {
    await rememberMe(owner)
    await db.cards.put(card())
    vi.mocked(accounts.fetchMe).mockResolvedValue(null)

    await expect(loadMe()).resolves.toBeNull()
    expect(await db.cards.count()).toBe(0)
    expect(await readMeta('me')).toBeUndefined()
  })
})

describe('session end', () => {
  it('keeps unsent changes when the session expires', async () => {
    await rememberMe(owner)
    await db.outbox.add({ cardId: 'c1', op: { type: 'delete' }, attempts: 0 })
    vi.mocked(accounts.fetchMe).mockResolvedValue(null)

    await loadMe()

    expect(await db.outbox.count()).toBe(1)
  })

  it('changes nothing when the check was cancelled by a fresh sign-in', async () => {
    await rememberMe(owner)
    await db.cards.put(card())
    vi.mocked(accounts.fetchMe).mockResolvedValue(null)
    const controller = new AbortController()
    controller.abort()

    await loadMe(controller.signal)

    expect(await db.cards.count()).toBe(1)
  })

  it('falls back to the remembered user on server errors', async () => {
    await rememberMe(owner)
    vi.mocked(accounts.fetchMe).mockRejectedValue(new ApiError(502, 'http_error', 'bad gateway'))
    await expect(loadMe()).resolves.toEqual(owner)
  })
})

describe('rememberMe', () => {
  it("drops the previous user's cards when someone else signs in", async () => {
    await rememberMe(owner)
    await db.cards.put(card())

    await rememberMe(member)

    expect(await db.cards.count()).toBe(0)
    expect(await readMeta('me')).toEqual(member)
  })
})
