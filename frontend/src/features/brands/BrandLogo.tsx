import { useState } from 'react'
import { brandLogoUrl } from '../../api/brands'

interface BrandLogoProps {
  readonly logoId: string | null
  /** Tailwind size class, e.g. `size-8`. */
  readonly className?: string
}

/**
 * The brand's logo on a white chip, so it reads on any card colour. Decorative:
 * the store name is always shown next to it. Renders nothing when there is no
 * logo or it cannot be loaded (e.g. offline before it was ever cached).
 */
export function BrandLogo({ logoId, className = 'size-8' }: BrandLogoProps) {
  const [failedId, setFailedId] = useState<string | null>(null)
  if (!logoId || failedId === logoId) return null
  return (
    <img
      src={brandLogoUrl(logoId)}
      alt=""
      loading="lazy"
      decoding="async"
      onError={() => setFailedId(logoId)}
      className={`${className} shrink-0 rounded-lg bg-white object-contain p-1 shadow-sm ring-1 ring-black/5`}
    />
  )
}
