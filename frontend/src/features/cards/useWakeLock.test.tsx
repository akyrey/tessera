import { renderHook, waitFor } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { useWakeLock } from './useWakeLock'

function stubWakeLock(request: () => Promise<unknown>) {
  Object.defineProperty(navigator, 'wakeLock', { value: { request: vi.fn(request) }, configurable: true })
  return (navigator as unknown as { wakeLock: { request: ReturnType<typeof vi.fn> } }).wakeLock.request
}

afterEach(() => {
  Reflect.deleteProperty(navigator, 'wakeLock')
})

describe('useWakeLock', () => {
  it('holds a screen lock while mounted and releases it on unmount', async () => {
    const release = vi.fn().mockResolvedValue(undefined)
    const request = stubWakeLock(() => Promise.resolve({ release }))

    const { unmount } = renderHook(() => useWakeLock())
    await waitFor(() => expect(request).toHaveBeenCalledWith('screen'))
    await Promise.resolve()
    unmount()

    expect(release).toHaveBeenCalled()
  })

  it('asks again when the page becomes visible', async () => {
    const request = stubWakeLock(() => Promise.resolve({ release: vi.fn() }))
    renderHook(() => useWakeLock())
    await waitFor(() => expect(request).toHaveBeenCalledTimes(1))

    document.dispatchEvent(new Event('visibilitychange'))

    await waitFor(() => expect(request).toHaveBeenCalledTimes(2))
  })

  it('tolerates refusals and missing support', async () => {
    const request = stubWakeLock(() => Promise.reject(new Error('NotAllowedError')))
    const { unmount } = renderHook(() => useWakeLock())
    await waitFor(() => expect(request).toHaveBeenCalled())
    unmount()

    Reflect.deleteProperty(navigator, 'wakeLock')
    expect(() => renderHook(() => useWakeLock()).unmount()).not.toThrow()
  })
})
