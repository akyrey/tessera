import { useId, useState } from 'react'
import type { BarcodeFormat } from '../../api/generated/BarcodeFormat'
import { Button } from '../../components/Button'
import { CameraScanner } from './CameraScanner'
import { type ScanResult, scanImage } from './zxing'

interface ScanControlsProps {
  readonly onScanned: (format: BarcodeFormat, text: string) => void
}

/** "Scan with camera" and "Scan from photo", filling the form with what was read. */
export function ScanControls({ onScanned }: ScanControlsProps) {
  const inputId = useId()
  const [cameraOpen, setCameraOpen] = useState(false)
  const [busy, setBusy] = useState(false)
  const [message, setMessage] = useState<string | null>(null)

  const handle = (result: ScanResult) => {
    setCameraOpen(false)
    if (result.status === 'found') {
      setMessage(null)
      onScanned(result.format, result.text)
    } else if (result.status === 'unsupported') {
      setMessage(`This barcode type (${result.formatName}) is not supported. Enter the number by hand.`)
    } else {
      setMessage('No barcode found. Try a sharper photo with the barcode filling most of it.')
    }
  }

  const scanFile = async (file: File | undefined) => {
    if (!file) return
    setBusy(true)
    try {
      handle(await scanImage(file))
    } catch {
      setMessage('This image could not be read. Try a JPEG or PNG.')
    } finally {
      setBusy(false)
    }
  }

  return (
    <div className="flex flex-col gap-2">
      <div className="grid grid-cols-2 gap-2">
        <Button variant="secondary" onClick={() => setCameraOpen(true)}>
          Scan with camera
        </Button>
        <label
          htmlFor={inputId}
          aria-busy={busy || undefined}
          className="inline-flex min-h-11 cursor-pointer items-center justify-center rounded-xl bg-white px-4 font-medium ring-1 ring-brand-900/15 hover:bg-brand-50"
        >
          {busy ? 'Reading…' : 'Scan from photo'}
        </label>
        <input
          id={inputId}
          type="file"
          accept="image/*"
          aria-label="Barcode photo"
          className="sr-only"
          onChange={(event) => {
            void scanFile(event.target.files?.[0])
            event.target.value = ''
          }}
        />
      </div>
      {message && (
        <p role="alert" className="text-sm text-amber-900">
          {message}
        </p>
      )}
      {cameraOpen && <CameraScanner onResult={handle} onClose={() => setCameraOpen(false)} />}
    </div>
  )
}
