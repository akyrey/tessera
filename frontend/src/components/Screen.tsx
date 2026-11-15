import type { ReactNode } from 'react'

interface ScreenProps {
  readonly children: ReactNode
  readonly centered?: boolean
}

/** Full-height page respecting the iPhone notch and home indicator. */
export function Screen({ children, centered = false }: ScreenProps) {
  const layout = centered ? 'justify-center' : 'justify-start'
  return (
    <main
      className={`mx-auto flex min-h-dvh w-full max-w-md flex-col ${layout} gap-6 px-4 pt-[max(1.5rem,env(safe-area-inset-top))] pb-[max(1.5rem,env(safe-area-inset-bottom))]`}
    >
      {children}
    </main>
  )
}
