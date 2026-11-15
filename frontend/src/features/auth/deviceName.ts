const DEVICE_PATTERNS: ReadonlyArray<readonly [RegExp, string]> = [
  [/iPhone/, 'iPhone'],
  [/iPad/, 'iPad'],
  [/Android/, 'Android'],
  [/Macintosh|Mac OS X/, 'Mac'],
  [/Windows/, 'Windows'],
  [/CrOS/, 'Chromebook'],
  [/Linux/, 'Linux'],
]

/** A friendly default name for a new passkey, e.g. "iPhone". */
export function guessDeviceName(userAgent: string = navigator.userAgent): string {
  return DEVICE_PATTERNS.find(([pattern]) => pattern.test(userAgent))?.[1] ?? 'Passkey'
}
