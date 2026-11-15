import type { ReactNode } from 'react'

interface SectionProps {
  readonly title: string
  readonly children: ReactNode
}

export function Section({ title, children }: SectionProps) {
  return (
    <section
      aria-label={title}
      className="flex flex-col gap-3 rounded-2xl bg-white p-4 ring-1 ring-brand-900/10"
    >
      <h2 className="text-lg font-semibold">{title}</h2>
      {children}
    </section>
  )
}
