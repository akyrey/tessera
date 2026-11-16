import { describe, expect, it } from 'vitest'
import { toScanResult } from './zxing'

const result = (format: string, text: string, isValid = true) => ({ format, text, isValid })

describe('toScanResult', () => {
  it.each([
    ['EAN13', 'ean13'],
    ['UPCA', 'upca'],
    ['Code39Std', 'code39'],
    ['ITF14', 'itf'],
    ['QRCode', 'qr'],
    ['AztecCode', 'aztec'],
    ['DataMatrix', 'datamatrix'],
  ])('maps %s to %s', (zxingFormat, ours) => {
    expect(toScanResult([result(zxingFormat, '123')])).toEqual({ status: 'found', format: ours, text: '123' })
  })

  it('reports formats we cannot show', () => {
    expect(toScanResult([result('MaxiCode', 'x')])).toEqual({ status: 'unsupported', formatName: 'MaxiCode' })
  })

  it('ignores invalid or empty reads', () => {
    expect(toScanResult([])).toEqual({ status: 'none' })
    expect(toScanResult([result('EAN13', '1', false), result('QRCode', '')])).toEqual({ status: 'none' })
  })
})
