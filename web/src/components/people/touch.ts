// How a person stands, in words used by the list and the person page: when we last spoke, how often we mean to,
// and whether a birthday is close.
import type { PersonView } from '../../core/generated/PersonView'

export function lastTouch(p: PersonView): string {
  if (p.days_since_contact === null) return 'Never'
  if (p.days_since_contact === 0) return 'Today'
  if (p.days_since_contact === 1) return 'Yesterday'
  if (p.days_since_contact < 60) return `${p.days_since_contact}d ago`
  return `${Math.round(p.days_since_contact / 30)}mo ago`
}

export function cadence(p: PersonView): string {
  const n = p.contact_every_days
  if (!n) return ''
  if (n === 7) return 'Weekly'
  if (n === 14) return 'Every 2 weeks'
  if (n === 30 || n === 31) return 'Monthly'
  return `Every ${n}d`
}

/** Days past the cadence (positive when late). */
export function lateBy(p: PersonView): number | null {
  if (!p.contact_every_days) return null
  if (p.days_since_contact === null) return null
  return p.days_since_contact - p.contact_every_days
}

export const SOON = 30
export function birthdaySoon(p: PersonView): boolean {
  return p.days_to_birthday !== null && p.days_to_birthday <= SOON
}
export function birthdayText(p: PersonView): string {
  const d = p.days_to_birthday
  if (d === null) return ''
  return d === 0 ? 'Birthday today' : d === 1 ? 'Birthday tomorrow' : `Birthday in ${d}d`
}
