// Times and schedules the agent pages show in words: a cron expression as a sentence,
// a server timestamp ("YYYY-MM-DD HH:MM[:SS[.mmm]]", local time) as "Today 20:19" or "2h ago",
// and an agent's Markdown reply as plain text for a short preview.

const DAYS = ['Sundays', 'Mondays', 'Tuesdays', 'Wednesdays', 'Thursdays', 'Fridays', 'Saturdays']
const MONTHS = ['January', 'February', 'March', 'April', 'May', 'June', 'July', 'August', 'September', 'October', 'November', 'December']
const pad = (n: number) => String(n).padStart(2, '0')
const isNum = (s: string) => /^\d+$/.test(s)

function dayList(field: string): string | null {
  if (field === '1-5') return 'Weekdays'
  if (field === '0,6' || field === '6,0') return 'Weekends'
  const parts = field.split(',')
  if (!parts.every((p) => isNum(p) && Number(p) <= 7)) return null
  const names = parts.map((p) => DAYS[Number(p) % 7])
  return names.length === 1 ? names[0] : names.slice(0, -1).join(', ') + ' and ' + names[names.length - 1]
}

function ordinal(n: number): string {
  const s = n % 100 >= 11 && n % 100 <= 13 ? 'th' : ['th', 'st', 'nd', 'rd'][n % 10] ?? 'th'
  return n + s
}

/** A five-field cron expression in words, or null when it is not one of the shapes people usually write. */
export function cronWords(cron: string): string | null {
  const f = cron.trim().split(/\s+/)
  if (f.length !== 5) return null
  const [min, hour, dom, mon, dow] = f
  const anyDate = dom === '*' && mon === '*'
  if (min === '*' && hour === '*' && anyDate && dow === '*') return 'Every minute'
  const everyMin = /^\*\/(\d+)$/.exec(min)
  if (everyMin && hour === '*' && anyDate && dow === '*') return `Every ${everyMin[1]} minutes`
  if (isNum(min) && hour === '*' && anyDate && dow === '*') return Number(min) === 0 ? 'Every hour' : `Every hour at :${pad(Number(min))}`
  const everyHour = /^\*\/(\d+)$/.exec(hour)
  if (isNum(min) && everyHour && anyDate && dow === '*') {
    return `Every ${everyHour[1]} hours` + (Number(min) ? ` at :${pad(Number(min))}` : '')
  }
  if (!isNum(min)) return null
  const hours = hour.split(',')
  if (!hours.every(isNum)) return null
  const at = hours.map((h) => `${pad(Number(h))}:${pad(Number(min))}`).join(', ')
  if (anyDate && dow === '*') return `Every day at ${at}`
  if (anyDate) {
    const days = dayList(dow)
    return days ? `${days} at ${at}` : null
  }
  if (isNum(dom) && mon === '*' && dow === '*') return `Monthly on the ${ordinal(Number(dom))} at ${at}`
  if (isNum(dom) && isNum(mon) && dow === '*' && MONTHS[Number(mon) - 1]) return `Every ${MONTHS[Number(mon) - 1]} ${Number(dom)} at ${at}`
  return null
}

function parse(stamp: string): Date | null {
  const m = /^(\d{4})-(\d{2})-(\d{2})[ T](\d{2}):(\d{2})(?::(\d{2}))?/.exec(stamp)
  if (!m) return null
  return new Date(+m[1], +m[2] - 1, +m[3], +m[4], +m[5], +(m[6] ?? 0))
}

const dayMonth = new Intl.DateTimeFormat('en-US', { month: 'short', day: 'numeric' })
const weekday = new Intl.DateTimeFormat('en-US', { weekday: 'short' })

/** "Today 20:19", "Yesterday 08:00", "Tue 09:00", "Nov 3 09:00" — past or future. */
export function when(stamp: string | null | undefined, now = new Date()): string {
  if (!stamp) return ''
  const d = parse(stamp)
  if (!d) return stamp
  const time = `${pad(d.getHours())}:${pad(d.getMinutes())}`
  const start = (x: Date) => new Date(x.getFullYear(), x.getMonth(), x.getDate()).getTime()
  const days = Math.round((start(d) - start(now)) / 86_400_000)
  if (days === 0) return `Today ${time}`
  if (days === -1) return `Yesterday ${time}`
  if (days === 1) return `Tomorrow ${time}`
  if (Math.abs(days) < 7) return `${weekday.format(d)} ${time}`
  return `${dayMonth.format(d)} ${time}`
}

/** "just now", "5m ago", "2h ago", then `when`. In the future: "in 40m", "in 3h". */
export function ago(stamp: string | null | undefined, now = new Date()): string {
  if (!stamp) return ''
  const d = parse(stamp)
  if (!d) return stamp
  const mins = Math.round((now.getTime() - d.getTime()) / 60_000)
  if (mins >= 0) {
    if (mins < 1) return 'just now'
    if (mins < 60) return `${mins}m ago`
    if (mins < 12 * 60) return `${Math.round(mins / 60)}h ago`
  } else {
    if (-mins < 60) return `in ${-mins}m`
    if (-mins < 12 * 60) return `in ${Math.round(-mins / 60)}h`
  }
  return when(stamp, now)
}

/** How long a run took, from milliseconds. */
export function took(ms: number | null | undefined): string {
  if (ms === null || ms === undefined) return ''
  if (ms < 1000) return `${ms}ms`
  const s = Math.round(ms / 1000)
  return s < 60 ? `${s}s` : `${Math.floor(s / 60)}m ${pad(s % 60)}s`
}

/** Markdown as one line of plain text: no emphasis marks, code ticks, heading hashes or link targets. */
export function plain(md: string | null | undefined): string {
  if (!md) return ''
  return md
    .replace(/```[\s\S]*?```/g, ' ')
    .replace(/!\[([^\]]*)\]\([^)]*\)/g, '$1')
    .replace(/\[([^\]]*)\]\([^)]*\)/g, '$1')
    .replace(/^\s{0,3}(#{1,6}|>|[-*+]|\d+\.)\s+/gm, '')
    .replace(/(\*\*|__|~~)(.+?)\1/g, '$2')
    .replace(/(^|[^\w*])[*_](\S(?:.*?\S)?)[*_](?![\w*])/g, '$1$2')
    .replace(/`/g, '')
    .replace(/\s+/g, ' ')
    .trim()
}
