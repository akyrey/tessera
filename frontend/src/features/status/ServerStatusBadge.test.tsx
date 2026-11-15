import { render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import { ApiError } from '../../api/client'
import * as meta from '../../api/meta'
import { ServerStatusBadge } from './ServerStatusBadge'
import { useServerStatus } from './useServerStatus'

vi.mock('../../api/meta')

function Probe() {
  return <ServerStatusBadge status={useServerStatus()} />
}

describe('server status', () => {
  it('shows the version once the server answers', async () => {
    vi.mocked(meta.fetchAppInfo).mockResolvedValue({ name: 'tessera', version: '0.2.0' })

    render(<Probe />)

    expect(screen.getByRole('status')).toHaveTextContent('Connecting')
    expect(await screen.findByText('Server online · v0.2.0')).toBeInTheDocument()
  })

  it('reports an unreachable server', async () => {
    vi.mocked(meta.fetchAppInfo).mockRejectedValue(
      new ApiError(0, 'network_error', 'Cannot reach the server'),
    )

    render(<Probe />)

    expect(await screen.findByText('Offline: Cannot reach the server')).toBeInTheDocument()
  })

  it('hides unexpected failure details', async () => {
    vi.spyOn(console, 'error').mockImplementation(() => undefined)
    vi.mocked(meta.fetchAppInfo).mockRejectedValue(new Error('boom'))

    render(<Probe />)

    expect(await screen.findByText('Offline: Something went wrong')).toBeInTheDocument()
  })
})
