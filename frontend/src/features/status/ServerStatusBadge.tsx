import type { ServerStatus } from './useServerStatus'

const STYLES: Record<ServerStatus['state'], string> = {
  loading: 'bg-slate-100 text-slate-600',
  online: 'bg-emerald-100 text-emerald-800',
  offline: 'bg-amber-100 text-amber-900',
}

function label(status: ServerStatus): string {
  switch (status.state) {
    case 'loading':
      return 'Connecting…'
    case 'online':
      return `Server online · v${status.info.version}`
    case 'offline':
      return `Offline: ${status.reason}`
  }
}

export function ServerStatusBadge({ status }: { readonly status: ServerStatus }) {
  return (
    <p role="status" className={`rounded-full px-4 py-1.5 text-sm font-medium ${STYLES[status.state]}`}>
      {label(status)}
    </p>
  )
}
