import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { BrandLogo } from './BrandLogo'

const logo = () => screen.queryByRole('presentation')

describe('BrandLogo', () => {
  it('shows the stored logo', () => {
    render(<BrandLogo logoId="logo-1" />)
    expect(logo()).toHaveAttribute('src', '/api/brands/logos/logo-1')
  })

  it('renders nothing without a logo', () => {
    render(<BrandLogo logoId={null} />)
    expect(logo()).toBeNull()
  })

  it('hides a logo that fails to load, but tries a new one', () => {
    const { rerender } = render(<BrandLogo logoId="logo-1" />)
    fireEvent.error(logo()!)
    expect(logo()).toBeNull()

    rerender(<BrandLogo logoId="logo-2" />)
    expect(logo()).toHaveAttribute('src', '/api/brands/logos/logo-2')
  })
})
