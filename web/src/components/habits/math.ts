// What a habit's numbers say, once: how often it is meant to happen, its best run in the days we have, and counts.
import type { HabitStatus } from '../../core/generated/HabitStatus'

export function cadence(h: HabitStatus): string {
  return h.target_per_week >= 7 ? 'Every day' : `${h.target_per_week} days a week`
}

/** The longest run of done days in `recent` (the last 84 days). */
export function bestStreak(recent: number[]): number {
  let best = 0, run = 0
  for (const v of recent) {
    run = v > 0 ? run + 1 : 0
    best = Math.max(best, run)
  }
  return best
}

/** Done days among the last `n`. */
export function doneIn(recent: number[], n: number): number {
  return recent.slice(-n).filter((v) => v > 0).length
}

/** Done days per week (Monday to Sunday), oldest first, ending with this week; `end` is today. */
export function weeks(recent: number[], end: string): { start: string; done: number; total: number }[] {
  const [y, m, d] = end.split('-').map(Number)
  const today = new Date(y, m - 1, d)
  const out: { start: string; done: number; total: number }[] = []
  const iso = (x: Date) => `${x.getFullYear()}-${String(x.getMonth() + 1).padStart(2, '0')}-${String(x.getDate()).padStart(2, '0')}`
  recent.forEach((v, i) => {
    const day = new Date(today)
    day.setDate(today.getDate() - (recent.length - 1 - i))
    const mon = new Date(day)
    mon.setDate(day.getDate() - ((day.getDay() + 6) % 7))
    const key = iso(mon)
    const last = out[out.length - 1]
    if (last?.start === key) { last.total++; if (v > 0) last.done++ } else out.push({ start: key, done: v > 0 ? 1 : 0, total: 1 })
  })
  return out
}
