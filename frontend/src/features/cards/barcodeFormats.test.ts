import { describe, expect, it } from 'vitest'
import type { BarcodeFormat } from '../../api/generated/BarcodeFormat'
import { BARCODE_FORMATS } from '../../api/schemas'
import { FORMAT_INFO, barcodeText, checkCardNumber, gtinCheckDigit } from './barcodeFormats'

const valid = (format: BarcodeFormat, raw: string) => checkCardNumber(format, raw)

describe('checkCardNumber', () => {
  it('computes GS1 check digits', () => {
    expect(gtinCheckDigit('400638133393')).toBe(1)
    expect(gtinCheckDigit('9638507')).toBe(4)
    expect(gtinCheckDigit('03600029145')).toBe(2)
  })

  it.each<[BarcodeFormat, string, string]>([
    ['ean13', ' 4 006381 333931 ', '4006381333931'],
    ['ean8', '96385074', '96385074'],
    ['upca', '036000291452', '036000291452'],
    ['itf', '1234 5678', '12345678'],
    ['code128', ' Abc-123 ', 'Abc-123'],
    ['code39', 'abc-12', 'ABC-12'],
    ['codabar', 'a40156b', 'A40156B'],
    ['codabar', '12-34$56', '12-34$56'],
    ['qr', ' https://x.example/1 ', 'https://x.example/1'],
    ['none', 'Ünï 12', 'Ünï 12'],
  ])('accepts %s %j', (format, raw, expected) => {
    expect(valid(format, raw)).toEqual({ ok: true, value: expected })
  })

  it.each<[BarcodeFormat, string, string]>([
    ['ean13', '400638133393', 'exactly 13 digits'],
    ['ean13', '4006381333932', 'last digit'],
    ['ean8', '96385075', 'EAN-8'],
    ['upca', '4006381333931', '12 digits'],
    ['itf', '123', 'even'],
    ['code128', 'caffè', 'ASCII'],
    ['code39', 'AB_12', 'Code 39'],
    ['code39', 'A'.repeat(44), '43'],
    ['codabar', 'A12E', 'Codabar'],
    ['pdf417', 'x'.repeat(513), '512'],
    ['aztec', 'a\u0000b', 'invalid characters'],
    ['qr', '   ', 'required'],
  ])('rejects %s %j', (format, raw, message) => {
    const result = valid(format, raw)
    expect(result.ok).toBe(false)
    expect(result.ok ? '' : result.message).toContain(message)
  })
})

describe('format metadata', () => {
  it('describes every format the API knows', () => {
    expect(Object.keys(FORMAT_INFO).sort()).toEqual([...BARCODE_FORMATS].sort())
  })

  it('wraps bare Codabar numbers in start/stop characters', () => {
    expect(barcodeText('codabar', '40156')).toBe('A40156A')
    expect(barcodeText('codabar', 'B40156C')).toBe('B40156C')
    expect(barcodeText('ean13', '4006381333931')).toBe('4006381333931')
  })
})
