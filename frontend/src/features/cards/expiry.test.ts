import { describe, expect, it } from 'vitest'
import { daysUntil, expiryLabel, expiryState } from './expiry'

const today = new Date(2027, 2, 10, 18, 30) // 10 March 2027, evening

describe('expiry', () => {
  it('counts calendar days in local time', () => {
    expect(daysUntil('2027-03-10', today)).toBe(0)
    expect(daysUntil('2027-03-11', today)).toBe(1)
    expect(daysUntil('2027-03-09', today)).toBe(-1)
    expect(daysUntil('2027-04-10', today)).toBe(31)
  })

  it('classifies cards', () => {
    expect(expiryState(null, today)).toBe('none')
    expect(expiryState('2027-03-09', today)).toBe('expired')
    expect(expiryState('2027-03-17', today)).toBe('soon')
    expect(expiryState('2027-03-18', today)).toBe('valid')
  })

  it('labels the remaining time', () => {
    expect(expiryLabel('2027-03-09', today)).toBe('Expired')
    expect(expiryLabel('2027-03-10', today)).toBe('Expires today')
    expect(expiryLabel('2027-03-11', today)).toBe('Expires tomorrow')
    expect(expiryLabel('2027-03-15', today)).toBe('Expires in 5 days')
    expect(expiryLabel('2027-12-31', today)).toBe('Valid until 31 Dec 2027')
  })
})
