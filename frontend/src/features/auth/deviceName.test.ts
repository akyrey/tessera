import { describe, expect, it } from 'vitest'
import { guessDeviceName } from './deviceName'

describe('guessDeviceName', () => {
  it.each([
    ['Mozilla/5.0 (iPhone; CPU iPhone OS 19_0 like Mac OS X)', 'iPhone'],
    ['Mozilla/5.0 (iPad; CPU OS 19_0 like Mac OS X)', 'iPad'],
    ['Mozilla/5.0 (Linux; Android 16; Pixel 9)', 'Android'],
    ['Mozilla/5.0 (Macintosh; Intel Mac OS X 15_0)', 'Mac'],
    ['Mozilla/5.0 (Windows NT 10.0; Win64; x64)', 'Windows'],
    ['Mozilla/5.0 (X11; CrOS x86_64 15000.0.0)', 'Chromebook'],
    ['Mozilla/5.0 (X11; Linux x86_64)', 'Linux'],
    ['curl/8.0', 'Passkey'],
  ])('%s → %s', (userAgent, expected) => {
    expect(guessDeviceName(userAgent)).toBe(expected)
  })
})
