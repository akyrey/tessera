import { describe, expect, it, vi } from 'vitest'
import * as client from './client'
import { fetchAppInfo } from './meta'

describe('fetchAppInfo', () => {
  it('requests the version endpoint with the caller signal', async () => {
    const getJson = vi.spyOn(client, 'getJson').mockResolvedValue({ name: 'tessera', version: '1.2.3' })
    const controller = new AbortController()

    await expect(fetchAppInfo(controller.signal)).resolves.toEqual({ name: 'tessera', version: '1.2.3' })
    expect(getJson).toHaveBeenCalledWith('/api/version', expect.anything(), controller.signal)
  })
})
