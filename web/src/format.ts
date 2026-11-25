// How numbers, money and days read on the page (Vietnamese).

export function money(amount: number, currency: string): string {
  try {
    return new Intl.NumberFormat('vi-VN', { style: 'currency', currency, maximumFractionDigits: currency === 'VND' ? 0 : 2 }).format(amount)
  } catch {
    return `${amount.toLocaleString('vi-VN')} ${currency}`
  }
}

/** 1,2tr, 450k: short money for axes and tight spots. */
export function compact(amount: number): string {
  const a = Math.abs(amount)
  const f = (n: number) => n.toLocaleString('vi-VN', { maximumFractionDigits: 1 })
  if (a >= 1e9) return f(amount / 1e9) + 'tỷ'
  if (a >= 1e6) return f(amount / 1e6) + 'tr'
  if (a >= 1e3) return f(amount / 1e3) + 'k'
  return f(amount)
}

export function num(n: number): string {
  return n.toLocaleString('vi-VN', { maximumFractionDigits: 2 })
}

const WEEKDAYS = ['Chủ nhật', 'Thứ hai', 'Thứ ba', 'Thứ tư', 'Thứ năm', 'Thứ sáu', 'Thứ bảy']

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

/** "Thứ tư, 25/11" */
export function longDay(day: string): string {
  const d = parseDay(day)
  return `${WEEKDAYS[d.getDay()]}, ${d.getDate()}/${d.getMonth() + 1}`
}

export function shortDay(day: string): string {
  const d = parseDay(day)
  return `${d.getDate()}/${d.getMonth() + 1}`
}

/** "hôm nay", "mai", "3 ngày nữa", "quá 2 ngày" */
export function relativeDay(day: string, from = todayIso()): string {
  const diff = Math.round((parseDay(day).getTime() - parseDay(from).getTime()) / 86_400_000)
  if (diff === 0) return 'hôm nay'
  if (diff === 1) return 'mai'
  if (diff === -1) return 'hôm qua'
  if (diff > 1) return diff < 7 ? `${WEEKDAYS[parseDay(day).getDay()].toLowerCase()}` : shortDay(day)
  return `trễ ${-diff} ngày`
}

export function greeting(): string {
  const h = new Date().getHours()
  if (h < 11) return 'Chào buổi sáng'
  if (h < 14) return 'Chào buổi trưa'
  if (h < 18) return 'Chào buổi chiều'
  return 'Chào buổi tối'
}
