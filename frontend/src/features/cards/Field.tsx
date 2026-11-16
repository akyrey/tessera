import type { ReactNode } from 'react'

export const INPUT_CLASS =
  'min-h-11 rounded-xl bg-white px-4 py-2 ring-1 ring-brand-900/15 focus:ring-2 focus:ring-brand-700 focus:outline-none'

export interface FieldControlProps {
  readonly id: string
  readonly 'aria-invalid'?: boolean
  readonly 'aria-describedby'?: string
}

interface FieldProps {
  readonly label: string
  readonly error?: string | null
  readonly children: (props: FieldControlProps) => ReactNode
}

/** Label, control and error message, wired together for screen readers. */
export function Field({ label, error, children }: FieldProps) {
  const id = `field-${label.toLowerCase().replace(/\W+/g, '-')}`
  const errorId = `${id}-error`
  return (
    <div className="flex flex-col gap-1">
      <label htmlFor={id} className="font-medium">
        {label}
      </label>
      {children({ id, ...(error ? { 'aria-invalid': true, 'aria-describedby': errorId } : {}) })}
      {error && (
        <p id={errorId} className="text-sm text-red-700">
          {error}
        </p>
      )}
    </div>
  )
}
