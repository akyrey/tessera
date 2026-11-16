import type { BarcodeFormat } from '../../api/generated/BarcodeFormat'

// Mirrors backend/src/cards/barcode.rs for instant feedback; the server stays authoritative.

export interface FormatInfo {
  readonly label: string
  /** bwip-js symbology id; `null` when no barcode is drawn. */
  readonly bcid: string | null
  readonly twoDimensional: boolean
  readonly inputMode: 'numeric' | 'text'
}

export const FORMAT_INFO: Readonly<Record<BarcodeFormat, FormatInfo>> = {
  ean13: { label: 'EAN-13 (13 digits)', bcid: 'ean13', twoDimensional: false, inputMode: 'numeric' },
  ean8: { label: 'EAN-8 (8 digits)', bcid: 'ean8', twoDimensional: false, inputMode: 'numeric' },
  upca: { label: 'UPC-A (12 digits)', bcid: 'upca', twoDimensional: false, inputMode: 'numeric' },
  code128: { label: 'Code 128', bcid: 'code128', twoDimensional: false, inputMode: 'text' },
  code39: { label: 'Code 39', bcid: 'code39', twoDimensional: false, inputMode: 'text' },
  itf: { label: 'Interleaved 2 of 5', bcid: 'interleaved2of5', twoDimensional: false, inputMode: 'numeric' },
  codabar: { label: 'Codabar', bcid: 'rationalizedCodabar', twoDimensional: false, inputMode: 'text' },
  qr: { label: 'QR code', bcid: 'qrcode', twoDimensional: true, inputMode: 'text' },
  pdf417: { label: 'PDF417', bcid: 'pdf417', twoDimensional: true, inputMode: 'text' },
  aztec: { label: 'Aztec', bcid: 'azteccode', twoDimensional: true, inputMode: 'text' },
  datamatrix: { label: 'Data Matrix', bcid: 'datamatrix', twoDimensional: true, inputMode: 'text' },
  none: { label: 'No barcode (number only)', bcid: null, twoDimensional: false, inputMode: 'text' },
}

const MAX_TEXT_CHARS = 512
const MAX_LINEAR_CHARS = 80
const MAX_CODE39_CHARS = 43
const GTIN_LENGTH: Partial<Record<BarcodeFormat, number>> = { ean13: 13, ean8: 8, upca: 12 }
const GTIN_NAME: Partial<Record<BarcodeFormat, string>> = { ean13: 'EAN-13', ean8: 'EAN-8', upca: 'UPC-A' }

export type NumberCheck =
  { readonly ok: true; readonly value: string } | { readonly ok: false; readonly message: string }

/** Normalises a card number for `format` and checks it, like the server does. */
export function checkCardNumber(format: BarcodeFormat, raw: string): NumberCheck {
  const value = normalize(format, raw)
  if (value === '') return fail('Card number is required')
  const message = problem(format, value)
  return message ? fail(message) : { ok: true, value }
}

function fail(message: string): NumberCheck {
  return { ok: false, message }
}

function normalize(format: BarcodeFormat, raw: string): string {
  switch (format) {
    case 'ean13':
    case 'ean8':
    case 'upca':
    case 'itf':
      return raw.replace(/\s/g, '')
    case 'code39':
    case 'codabar':
      return raw.trim().toUpperCase()
    default:
      return raw.trim()
  }
}

function problem(format: BarcodeFormat, value: string): string | null {
  switch (format) {
    case 'ean13':
    case 'ean8':
    case 'upca':
      return gtinProblem(format, value)
    case 'itf':
      return /^(\d\d)+$/.test(value) && value.length <= MAX_LINEAR_CHARS
        ? null
        : 'ITF numbers have an even number of digits (up to 80)'
    case 'code128':
      return /^[\x20-\x7e]+$/.test(value) && value.length <= MAX_LINEAR_CHARS
        ? null
        : 'Code 128 supports up to 80 plain ASCII characters'
    case 'code39':
      return /^[0-9A-Z \-.$/+%]+$/.test(value) && value.length <= MAX_CODE39_CHARS
        ? null
        : 'Code 39 supports up to 43 letters, digits and - . $ / + % or space'
    case 'codabar':
      return /^([A-D][0-9\-$:/.+]+[A-D]|[0-9\-$:/.+]+)$/.test(value) && value.length <= MAX_LINEAR_CHARS
        ? null
        : 'Codabar supports digits and - $ : / . +, optionally between start/stop letters A–D'
    default:
      return textProblem(value)
  }
}

function gtinProblem(format: BarcodeFormat, value: string): string | null {
  const length = GTIN_LENGTH[format] ?? 0
  const name = GTIN_NAME[format] ?? format
  if (value.length !== length || !/^\d+$/.test(value)) return `${name} numbers have exactly ${length} digits`
  const expected = gtinCheckDigit(value.slice(0, -1))
  return Number(value.at(-1)) === expected
    ? null
    : `This is not a valid ${name} number (the last digit does not match)`
}

/** GS1 mod-10 check digit: weights 3,1,3,1… from the rightmost digit of `body`. */
export function gtinCheckDigit(body: string): number {
  const sum = [...body]
    .reverse()
    .reduce((total, digit, index) => total + Number(digit) * (index % 2 === 0 ? 3 : 1), 0)
  return (10 - (sum % 10)) % 10
}

function textProblem(value: string): string | null {
  if ([...value].length > MAX_TEXT_CHARS) return `Card number must be at most ${MAX_TEXT_CHARS} characters`
  // eslint-disable-next-line no-control-regex
  if (/[\u0000-\u0008\u000b-\u001f\u007f-\u009f]/.test(value))
    return 'Card number contains invalid characters'
  return null
}

/** Text handed to bwip-js; Codabar needs explicit start/stop characters. */
export function barcodeText(format: BarcodeFormat, number: string): string {
  if (format === 'codabar' && !/^[A-D].*[A-D]$/.test(number)) return `A${number}A`
  return number
}
