export function Brand({ subtitle }: { readonly subtitle?: string }) {
  return (
    <header className="flex flex-col items-center gap-3 text-center">
      <img src="/favicon.svg" alt="" className="size-16" />
      <h1 className="text-3xl font-semibold tracking-tight">Tessera</h1>
      {subtitle && <p className="max-w-xs text-brand-900/70">{subtitle}</p>}
    </header>
  )
}
