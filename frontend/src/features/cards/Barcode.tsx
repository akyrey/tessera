import { useEffect, useState } from 'react'
import type { BarcodeFormat } from '../../api/generated/BarcodeFormat'
import { FORMAT_INFO, barcodeText } from './barcodeFormats'

interface BarcodeProps {
  readonly format: BarcodeFormat
  readonly value: string
}

/** The outcome for one input; a stale outcome counts as still loading. */
type Rendered = { readonly key: string } & ({ readonly src: string } | { readonly failed: true })

/** Draws the barcode as SVG (crisp at any size), on white with a quiet zone. */
export function Barcode({ format, value }: BarcodeProps) {
  const info = FORMAT_INFO[format]
  const key = `${format}:${value}`
  const [rendered, setRendered] = useState<Rendered | null>(null)

  useEffect(() => {
    if (!info.bcid) return
    let cancelled = false
    renderSvg(info.bcid, barcodeText(format, value), info.twoDimensional)
      .then((svg) => {
        if (!cancelled)
          setRendered({ key, src: `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}` })
      })
      .catch(() => {
        if (!cancelled) setRendered({ key, failed: true })
      })
    return () => {
      cancelled = true
    }
  }, [format, value, info, key])

  const state = rendered?.key === key ? rendered : null
  if (!info.bcid) return null
  if (state && 'failed' in state) {
    return (
      <p role="alert" className="rounded-xl bg-amber-50 p-4 text-center text-amber-900">
        This barcode cannot be drawn. Show the number below instead.
      </p>
    )
  }
  if (!state || !('src' in state))
    return <div aria-busy="true" className={info.twoDimensional ? 'aspect-square w-64' : 'h-32 w-full'} />
  return (
    <img
      src={state.src}
      alt={`${info.label} barcode`}
      className={`${info.twoDimensional ? 'aspect-square w-64 max-w-full' : 'h-32 w-full object-contain'} [image-rendering:pixelated]`}
    />
  )
}

/** bwip-js is large; load it only when a card is shown. */
async function renderSvg(bcid: string, text: string, twoDimensional: boolean): Promise<string> {
  const { toSVG } = await import('bwip-js/browser')
  return toSVG({
    bcid,
    text,
    scale: 3,
    ...(twoDimensional ? {} : { height: 20 }),
    paddingwidth: 10,
    paddingheight: 10,
    backgroundcolor: 'ffffff',
  })
}
