import { useEffect, useRef, useState } from 'react'
import { Button } from '../../components/Button'
import { cameraErrorMessage } from './cameraErrors'
import { type ScanResult, scanImage } from './zxing'

/** Time between decoded frames: fast enough to feel instant, light on the battery. */
const FRAME_INTERVAL_MS = 250

interface CameraScannerProps {
  readonly onResult: (result: Extract<ScanResult, { status: 'found' | 'unsupported' }>) => void
  readonly onClose: () => void
}

/** Live camera preview that stops at the first barcode it reads. */
export function CameraScanner({ onResult, onClose }: CameraScannerProps) {
  const video = useRef<HTMLVideoElement>(null)
  const [error, setError] = useState<string | null>(null)
  const done = useRef(onResult)
  useEffect(() => {
    done.current = onResult
  })

  useEffect(() => {
    let stream: MediaStream | null = null
    let timer: ReturnType<typeof setTimeout> | undefined
    let stopped = false
    const canvas = document.createElement('canvas')

    const scanFrame = async () => {
      const element = video.current
      const context = canvas.getContext('2d', { willReadFrequently: true })
      if (stopped || !element || !context) return
      if (element.videoWidth > 0) {
        canvas.width = element.videoWidth
        canvas.height = element.videoHeight
        context.drawImage(element, 0, 0)
        let result: ScanResult
        try {
          result = await scanImage(context.getImageData(0, 0, canvas.width, canvas.height), false)
        } catch {
          if (!stopped) setError('The scanner could not start. Enter the number by hand instead.')
          return
        }
        if (stopped) return
        if (result.status !== 'none') return done.current(result)
      }
      timer = setTimeout(() => void scanFrame(), FRAME_INTERVAL_MS)
    }

    const start = async () => {
      if (!navigator.mediaDevices?.getUserMedia) throw new Error('unsupported')
      const granted = await navigator.mediaDevices.getUserMedia({
        video: { facingMode: 'environment' },
        audio: false,
      })
      stream = granted
      // Closed while the permission prompt was open: release the camera at once.
      if (stopped || !video.current) return granted.getTracks().forEach((track) => track.stop())
      video.current.srcObject = granted
      await video.current.play()
      void scanFrame()
    }

    start().catch((failure: unknown) => {
      if (!stopped) setError(cameraErrorMessage(failure))
    })
    return () => {
      stopped = true
      clearTimeout(timer)
      stream?.getTracks().forEach((track) => track.stop())
    }
  }, [])

  return (
    <div
      role="dialog"
      aria-label="Scan barcode"
      className="fixed inset-0 z-10 flex flex-col bg-black text-white"
    >
      <video
        ref={video}
        muted
        playsInline
        aria-label="Camera preview"
        className="min-h-0 flex-1 object-cover"
      />
      <div className="flex flex-col gap-3 p-4 pb-[max(1rem,env(safe-area-inset-bottom))]">
        {error ? <p role="alert">{error}</p> : <p>Point the camera at the barcode on the card.</p>}
        <Button variant="secondary" onClick={onClose}>
          Cancel
        </Button>
      </div>
    </div>
  )
}
