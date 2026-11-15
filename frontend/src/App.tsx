import { ServerStatusBadge } from './features/status/ServerStatusBadge'
import { useServerStatus } from './features/status/useServerStatus'

export default function App() {
  const status = useServerStatus()

  return (
    <main className="flex min-h-dvh flex-col items-center justify-center gap-6 bg-brand-50 px-4 pt-[env(safe-area-inset-top)] pb-[env(safe-area-inset-bottom)] text-brand-900">
      <img src="/favicon.svg" alt="" className="size-20" />
      <h1 className="text-4xl font-semibold tracking-tight">Tessera</h1>
      <p className="max-w-xs text-center text-brand-900/70">Your loyalty cards, always at hand.</p>
      <ServerStatusBadge status={status} />
    </main>
  )
}
