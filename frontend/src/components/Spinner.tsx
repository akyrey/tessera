export function FullPageSpinner({ label = 'Loading…' }: { readonly label?: string }) {
  return (
    <div role="status" className="flex min-h-dvh items-center justify-center">
      <span
        aria-hidden="true"
        className="size-8 animate-spin rounded-full border-4 border-brand-700 border-t-transparent"
      />
      <span className="sr-only">{label}</span>
    </div>
  )
}
