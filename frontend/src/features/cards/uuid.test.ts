import { describe, expect, it } from 'vitest'
import { uuidv7 } from './uuid'

const UUID_V7 = /^[0-9a-f]{8}-[0-9a-f]{4}-7[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/

describe('uuidv7', () => {
  it('produces RFC 9562 version 7 ids', () => {
    expect(uuidv7()).toMatch(UUID_V7)
  })

  it('encodes the timestamp in the first 48 bits', () => {
    const id = uuidv7(0x0190_f0c8_1234)
    expect(id.startsWith('0190f0c8-1234-7')).toBe(true)
  })

  it('sorts by creation time and never repeats', () => {
    const earlier = uuidv7(1_000)
    const later = uuidv7(2_000)
    expect(earlier < later).toBe(true)
    expect(uuidv7(1_000)).not.toBe(earlier)
  })
})
