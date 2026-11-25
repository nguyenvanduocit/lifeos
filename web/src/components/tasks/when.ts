// When a task is for, in words and a tone: due dates past today are late; scheduled days are plans, never late.
import type { DocSummary } from '../../core/generated/DocSummary'
import { relativeDay, todayIso } from '../../format'

export interface When { text: string; tone: 'critical' | 'warning' | 'neutral'; day: string }

export function whenOf(t: DocSummary, today = todayIso()): When | null {
  if (t.due) {
    const late = t.due < today
    const days = Math.round((Date.parse(today) - Date.parse(t.due)) / 86_400_000)
    return {
      day: t.due,
      tone: late ? 'critical' : t.due === today ? 'warning' : 'neutral',
      text: late ? (days === 1 ? 'Due yesterday' : `${days} days late`) : t.due === today ? 'Due today' : `Due ${relativeDay(t.due, today)}`,
    }
  }
  if (t.scheduled) return { day: t.scheduled, tone: 'neutral', text: t.scheduled === today ? 'Today' : relativeDay(t.scheduled, today) }
  return null
}

/** The day a task belongs to on a calendar: its plan, else its deadline. */
export function dayOf(t: DocSummary): string | null {
  return t.scheduled ?? t.due
}
