const UNIX_MS_BYTES = 6

/** RFC 9562 UUIDv7: 48-bit Unix milliseconds + random bits, so ids sort by creation time. */
export function uuidv7(now: number = Date.now()): string {
  const bytes = crypto.getRandomValues(new Uint8Array(16))
  const timestamp = bytes.map((byte, index) => {
    if (index >= UNIX_MS_BYTES) return byte
    return Math.floor(now / 2 ** (8 * (UNIX_MS_BYTES - 1 - index))) % 256
  })
  const withVersion = timestamp.map((byte, index) => {
    if (index === 6) return (byte & 0x0f) | 0x70
    if (index === 8) return (byte & 0x3f) | 0x80
    return byte
  })
  const hex = [...withVersion].map((byte) => byte.toString(16).padStart(2, '0')).join('')
  return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`
}
