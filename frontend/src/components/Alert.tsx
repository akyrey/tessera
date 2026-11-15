import type { ReactNode } from 'react'

export function Alert({ children }: { readonly children: ReactNode }) {
  return (
    <p role="alert" className="rounded-xl bg-red-50 px-4 py-3 text-sm text-red-800 ring-1 ring-red-200">
      {children}
    </p>
  )
}
