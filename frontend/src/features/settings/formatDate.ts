const FORMATTER = new Intl.DateTimeFormat(undefined, { day: 'numeric', month: 'short', year: 'numeric' })

/** Formats a Unix timestamp (seconds) in the user's locale. */
export function formatDate(unixSeconds: number): string {
  return FORMATTER.format(new Date(unixSeconds * 1000))
}
