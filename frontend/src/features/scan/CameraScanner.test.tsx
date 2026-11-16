import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { CameraScanner } from './CameraScanner'
import { cameraErrorMessage } from './cameraErrors'
import * as zxing from './zxing'

vi.mock('./zxing', async (importOriginal) => ({
  ...(await importOriginal<typeof import('./zxing')>()),
  scanImage: vi.fn(),
}))

const stopTrack = vi.fn()

function stubCamera(getUserMedia: () => Promise<unknown>) {
  Object.defineProperty(navigator, 'mediaDevices', {
    value: { getUserMedia: vi.fn(getUserMedia) },
    configurable: true,
  })
}

afterEach(() => {
  Reflect.deleteProperty(navigator, 'mediaDevices')
})

describe('CameraScanner', () => {
  it('reads frames until a barcode is found, then stops the camera', async () => {
    stubCamera(() => Promise.resolve({ getTracks: () => [{ stop: stopTrack }] }))
    vi.spyOn(HTMLMediaElement.prototype, 'play').mockResolvedValue()
    vi.spyOn(HTMLVideoElement.prototype, 'videoWidth', 'get').mockReturnValue(4)
    vi.spyOn(HTMLVideoElement.prototype, 'videoHeight', 'get').mockReturnValue(4)
    vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue({
      drawImage: vi.fn(),
      getImageData: vi.fn(() => ({ width: 4, height: 4 })),
    } as never)
    vi.mocked(zxing.scanImage).mockResolvedValue({ status: 'found', format: 'ean13', text: '4006381333931' })
    const onResult = vi.fn()

    const { unmount } = render(<CameraScanner onResult={onResult} onClose={vi.fn()} />)

    await waitFor(() =>
      expect(onResult).toHaveBeenCalledWith({ status: 'found', format: 'ean13', text: '4006381333931' }),
    )
    unmount()
    expect(stopTrack).toHaveBeenCalled()
  })

  it('explains a denied camera and can be closed', async () => {
    const denied = Object.assign(new Error('denied'), { name: 'NotAllowedError' })
    stubCamera(() => Promise.reject(denied))
    const onClose = vi.fn()

    render(<CameraScanner onResult={vi.fn()} onClose={onClose} />)

    expect(await screen.findByRole('alert')).toHaveTextContent('Camera access was denied')
    await userEvent.click(screen.getByRole('button', { name: 'Cancel' }))
    expect(onClose).toHaveBeenCalled()
  })

  it('works without camera support', async () => {
    render(<CameraScanner onResult={vi.fn()} onClose={vi.fn()} />)
    expect(await screen.findByRole('alert')).toHaveTextContent('not available')
  })

  it('names common camera failures', () => {
    const named = (name: string) => cameraErrorMessage(Object.assign(new Error(), { name }))
    expect(named('NotFoundError')).toContain('No camera found')
    expect(named('SecurityError')).toContain('denied')
    expect(cameraErrorMessage('weird')).toContain('not available')
  })
})
