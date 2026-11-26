const locale = 'en-US'
const longDate = new Intl.DateTimeFormat(locale, { weekday: 'long', month: 'long', day: 'numeric' })
const shortDate = new Intl.DateTimeFormat(locale, { month: 'short', day: 'numeric' })
const shortDateYear = new Intl.DateTimeFormat(locale, { month: 'short', day: 'numeric', year: 'numeric' })
const weekday = new Intl.DateTimeFormat(locale, { weekday: 'long' })
const relative = new Intl.RelativeTimeFormat(locale, { numeric: 'auto' })
const compactNumber = new Intl.NumberFormat(locale, { notation: 'compact', maximumFractionDigits: 1 })
const number = new Intl.NumberFormat(locale, { maximumFractionDigits: 2 })

export function money(amount: number, currency: string): string {
  try {
    return new Intl.NumberFormat(locale, { style: 'currency', currency }).format(amount)
  } catch {
    return `${num(amount)} ${currency}`
  }
}

export function compact(amount: number): string {
  return compactNumber.format(amount)
}

export function num(n: number): string {
  return number.format(n)
}

export function parseDay(day: string): Date {
  const [y, m, d] = day.split('-').map(Number)
  return new Date(y, m - 1, d)
}

export function isoDay(date: Date): string {
  const p = (n: number) => String(n).padStart(2, '0')
  return `${date.getFullYear()}-${p(date.getMonth() + 1)}-${p(date.getDate())}`
}

export function todayIso(): string {
  return isoDay(new Date())
}

export function longDay(day: string): string {
  return longDate.format(parseDay(day))
}

export function shortDay(day: string): string {
  return shortDate.format(parseDay(day))
}

export function relativeDay(day: string, from = todayIso()): string {
  const date = parseDay(day)
  const start = parseDay(from)
  const diff = Math.round((date.getTime() - start.getTime()) / 86_400_000)
  if (diff <= 1) return relative.format(diff, 'day')
  if (diff < 7) return weekday.format(date)
  // A date in another year carries its year: "Dec 25" alone reads as this year's.
  return (date.getFullYear() === start.getFullYear() ? shortDate : shortDateYear).format(date)
}

export function greeting(): string {
  const h = new Date().getHours()
  if (h < 12) return 'Good morning'
  if (h < 18) return 'Good afternoon'
  return 'Good evening'
}

/** "₫14.5M", "$1.2K": an amount short enough for a stat or a chip, with its currency's sign. */
export function moneyShort(amount: number, currency: string): string {
  try {
    return new Intl.NumberFormat(locale, { style: 'currency', currency, notation: 'compact', maximumFractionDigits: 1 }).format(amount)
  } catch {
    return `${compact(amount)} ${currency}`
  }
}

/** ISO week number of a `YYYY-MM-DD` day. */
export function isoWeek(day: string): number {
  const d = parseDay(day)
  d.setDate(d.getDate() + 3 - ((d.getDay() + 6) % 7))
  const jan4 = new Date(d.getFullYear(), 0, 4)
  return 1 + Math.round(((d.getTime() - jan4.getTime()) / 86_400_000 - 3 + ((jan4.getDay() + 6) % 7)) / 7)
}

/** "Wednesday, 25th November", as Relay heads its home page. */
export function headDay(day: string): string {
  const d = parseDay(day)
  const n = d.getDate()
  const th = n % 100 >= 11 && n % 100 <= 13 ? 'th' : ({ 1: 'st', 2: 'nd', 3: 'rd' } as Record<number, string>)[n % 10] ?? 'th'
  return `${weekday.format(d)}, ${n}${th} ${new Intl.DateTimeFormat(locale, { month: 'long' }).format(d)}`
}

/** "2h ago", "3d ago", "just now" for an RFC 3339 or `YYYY-MM-DD HH:MM` time. */
export function ago(time: string, now = Date.now()): string {
  const t = new Date(time.includes('T') ? time : time.replace(' ', 'T')).getTime()
  const s = Math.max(0, Math.round((now - t) / 1000))
  if (s < 60) return 'just now'
  if (s < 3600) return `${Math.floor(s / 60)}m ago`
  if (s < 86_400) return `${Math.floor(s / 3600)}h ago`
  return `${Math.floor(s / 86_400)}d ago`
}
