/** Coupons expiring within this many days get their own section. */
export const EXPIRING_SOON_DAYS = 7
const MS_PER_DAY = 86_400_000

/** Whole days from `today` (local calendar) to `isoDate`; negative once expired. */
export function daysUntil(isoDate: string, today: Date): number {
  const [year = 0, month = 1, day = 1] = isoDate.split('-').map(Number)
  const target = Date.UTC(year, month - 1, day)
  const start = Date.UTC(today.getFullYear(), today.getMonth(), today.getDate())
  return Math.round((target - start) / MS_PER_DAY)
}

export type ExpiryState = 'none' | 'valid' | 'soon' | 'expired'

export function expiryState(expiresOn: string | null, today: Date): ExpiryState {
  if (!expiresOn) return 'none'
  const days = daysUntil(expiresOn, today)
  if (days < 0) return 'expired'
  return days <= EXPIRING_SOON_DAYS ? 'soon' : 'valid'
}

const dateFormat = new Intl.DateTimeFormat('en-GB', { day: 'numeric', month: 'short', year: 'numeric' })

/** "Expired", "Expires today", "Expires in 3 days", "Valid until 31 Mar 2027". */
export function expiryLabel(expiresOn: string, today: Date): string {
  const days = daysUntil(expiresOn, today)
  if (days < 0) return 'Expired'
  if (days === 0) return 'Expires today'
  if (days === 1) return 'Expires tomorrow'
  if (days <= EXPIRING_SOON_DAYS) return `Expires in ${days} days`
  const [year = 0, month = 1, day = 1] = expiresOn.split('-').map(Number)
  return `Valid until ${dateFormat.format(new Date(year, month - 1, day))}`
}
