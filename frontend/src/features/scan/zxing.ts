import type { BarcodeFormat } from '../../api/generated/BarcodeFormat'

/** zxing-cpp format names (canonical enum form) → our formats. */
const FORMAT_MAP: Readonly<Record<string, BarcodeFormat>> = {
  EAN13: 'ean13',
  EAN8: 'ean8',
  UPCA: 'upca',
  Code128: 'code128',
  Code39: 'code39',
  Code39Std: 'code39',
  ITF: 'itf',
  ITF14: 'itf',
  Codabar: 'codabar',
  QRCode: 'qr',
  QRCodeModel2: 'qr',
  PDF417: 'pdf417',
  CompactPDF417: 'pdf417',
  Aztec: 'aztec',
  AztecCode: 'aztec',
  DataMatrix: 'datamatrix',
}

/** Formats the reader looks for: fewer formats means faster, more reliable scans. */
const READ_FORMATS = [
  'EAN13',
  'EAN8',
  'UPCA',
  'Code128',
  'Code39',
  'ITF',
  'Codabar',
  'QRCode',
  'PDF417',
  'Aztec',
  'DataMatrix',
] as const

export type ScanResult =
  | { readonly status: 'found'; readonly format: BarcodeFormat; readonly text: string }
  | { readonly status: 'unsupported'; readonly formatName: string }
  | { readonly status: 'none' }

export function toScanResult(
  results: readonly { format: string; text: string; isValid: boolean }[],
): ScanResult {
  const result = results.find((r) => r.isValid && r.text !== '')
  if (!result) return { status: 'none' }
  const format = FORMAT_MAP[result.format]
  return format
    ? { status: 'found', format, text: result.text }
    : { status: 'unsupported', formatName: result.format }
}

type Reader = typeof import('zxing-wasm/reader')
let reader: Promise<Reader> | null = null

/**
 * Loads the decoder on first use. The WASM file is bundled and served by us:
 * the library's default CDN would be blocked by the CSP and leak usage.
 */
function loadReader(): Promise<Reader> {
  reader ??= Promise.all([
    import('zxing-wasm/reader'),
    import('zxing-wasm/reader/zxing_reader.wasm?url'),
  ]).then(([module, wasm]) => {
    module.prepareZXingModule({
      overrides: {
        locateFile: (path: string, prefix: string) => (path.endsWith('.wasm') ? wasm.default : prefix + path),
      },
    })
    return module
  })
  return reader
}

/** Large photos are scaled down first: barcodes stay readable and decoding stays fast. */
const MAX_SCAN_DIMENSION = 2000

/** Reads a barcode from a still image (photo, screenshot) or a video frame. */
export async function scanImage(input: Blob | ImageData, tryHarder = true): Promise<ScanResult> {
  const pixels = input instanceof Blob ? await decodeOnWhite(input) : input
  const { readBarcodes } = await loadReader()
  const results = await readBarcodes(pixels, { formats: [...READ_FORMATS], tryHarder, maxNumberOfSymbols: 1 })
  return toScanResult(results)
}

/**
 * Decodes with the browser (so HEIC photos from iPhones work too) onto a white
 * background: transparent screenshots would otherwise read as black on black.
 */
async function decodeOnWhite(image: Blob): Promise<ImageData> {
  const bitmap = await createImageBitmap(image)
  try {
    const scale = Math.min(1, MAX_SCAN_DIMENSION / Math.max(bitmap.width, bitmap.height))
    const canvas = document.createElement('canvas')
    canvas.width = Math.max(1, Math.round(bitmap.width * scale))
    canvas.height = Math.max(1, Math.round(bitmap.height * scale))
    const context = canvas.getContext('2d', { willReadFrequently: true })
    if (!context) throw new Error('2D canvas unavailable')
    context.fillStyle = '#ffffff'
    context.fillRect(0, 0, canvas.width, canvas.height)
    context.drawImage(bitmap, 0, 0, canvas.width, canvas.height)
    return context.getImageData(0, 0, canvas.width, canvas.height)
  } finally {
    bitmap.close()
  }
}
