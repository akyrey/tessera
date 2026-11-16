import { render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { Barcode } from './Barcode'

describe('Barcode', () => {
  it('renders a valid EAN-13 as an SVG image', async () => {
    render(<Barcode format="ean13" value="4006381333931" />)

    const image = await screen.findByRole('img', { name: 'EAN-13 (13 digits) barcode' })
    expect(image.getAttribute('src')).toMatch(/^data:image\/svg\+xml/)
  })

  it('renders 2D codes', async () => {
    render(<Barcode format="qr" value="https://example.com" />)
    expect(await screen.findByRole('img', { name: 'QR code barcode' })).toBeInTheDocument()
  })

  it('explains when the value cannot be encoded', async () => {
    render(<Barcode format="ean13" value="4006381333932" />)
    expect(await screen.findByRole('alert')).toHaveTextContent('cannot be drawn')
  })

  it('draws nothing for number-only cards', () => {
    const { container } = render(<Barcode format="none" value="123" />)
    expect(container).toBeEmptyDOMElement()
  })
})
