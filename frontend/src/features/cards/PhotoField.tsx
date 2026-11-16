import { useId, useState } from 'react'
import { Button } from '../../components/Button'
import { KEEP_PHOTO, type PhotoChange } from './photoChange'
import { PhotoError, preparePhoto, toDataUrl } from './photos'

interface PhotoFieldProps {
  readonly label: string
  /** What is stored now (server or unsynced local photo). */
  readonly currentSrc: string | null
  readonly change: PhotoChange
  readonly onChange: (change: PhotoChange) => void
}

export function PhotoField({ label, currentSrc, change, onChange }: PhotoFieldProps) {
  const inputId = useId()
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const preview = previewOf(change, currentSrc)

  const pick = async (file: File | undefined) => {
    if (!file) return
    setBusy(true)
    setError(null)
    try {
      const photo = await preparePhoto(file)
      onChange({ type: 'replace', photo, preview: await toDataUrl(photo) })
    } catch (failure) {
      setError(failure instanceof PhotoError ? failure.message : 'This photo could not be processed.')
    } finally {
      setBusy(false)
    }
  }

  return (
    <fieldset className="flex flex-col gap-2">
      <legend className="font-medium">{label}</legend>
      {preview && (
        <img src={preview} alt={label} className="max-h-48 self-start rounded-xl ring-1 ring-black/10" />
      )}
      <div className="flex gap-2">
        <label
          htmlFor={inputId}
          aria-busy={busy || undefined}
          className="inline-flex min-h-11 cursor-pointer items-center rounded-xl bg-white px-4 font-medium ring-1 ring-brand-900/15 hover:bg-brand-50"
        >
          {busy ? 'Processing…' : preview ? 'Replace photo' : 'Add photo'}
        </label>
        <input
          id={inputId}
          type="file"
          accept="image/*"
          aria-label={`${label} photo`}
          className="sr-only"
          onChange={(event) => {
            void pick(event.target.files?.[0])
            event.target.value = ''
          }}
        />
        {preview && (
          <Button
            variant="secondary"
            onClick={() =>
              onChange(change.type === 'replace' || !currentSrc ? KEEP_PHOTO : { type: 'remove' })
            }
          >
            Remove
          </Button>
        )}
      </div>
      {error && (
        <p role="alert" className="text-sm text-red-700">
          {error}
        </p>
      )}
    </fieldset>
  )
}

/** URL to show for the field: the new photo, the stored one, or none. */
function previewOf(change: PhotoChange, currentSrc: string | null): string | null {
  if (change.type === 'replace') return change.preview
  return change.type === 'remove' ? null : currentSrc
}
