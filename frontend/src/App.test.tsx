import { render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import App from './App'
import { ApiError } from './api/client'
import * as meta from './api/meta'

describe('App', () => {
  it('shows the server version once it is reachable', async () => {
    vi.spyOn(meta, 'fetchAppInfo').mockResolvedValue({ name: 'tessera', version: '0.1.0' })

    render(<App />)

    expect(screen.getByRole('heading', { name: 'Tessera' })).toBeInTheDocument()
    expect(screen.getByRole('status')).toHaveTextContent('Connecting')
    expect(await screen.findByText('Server online · v0.1.0')).toBeInTheDocument()
  })

  it('tells the user when the server cannot be reached', async () => {
    vi.spyOn(meta, 'fetchAppInfo').mockRejectedValue(
      new ApiError(0, 'network_error', 'Cannot reach the server'),
    )

    render(<App />)

    expect(await screen.findByText('Offline: Cannot reach the server')).toBeInTheDocument()
  })

  it('shows a generic message for unexpected failures', async () => {
    vi.spyOn(meta, 'fetchAppInfo').mockRejectedValue(new Error('boom'))

    render(<App />)

    expect(await screen.findByText('Offline: Something went wrong')).toBeInTheDocument()
  })
})
