import { isoDay, parseDay } from '../../format'

/** Days in a row with an entry, ending today (or yesterday, when today is still unwritten). */
export function streak(days: Set<string> | Map<string, unknown>, today: string): number {
  const d = parseDay(today)
  if (!days.has(today)) d.setDate(d.getDate() - 1)
  let n = 0
  while (days.has(isoDay(d))) {
    n++
    d.setDate(d.getDate() - 1)
  }
  return n
}
