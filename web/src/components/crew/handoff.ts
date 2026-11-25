// How a hand-off stands, in a word and a tone (docs/adr/0002): an agent's page and the automation page show it so.
import type { Handoff } from '../../core/generated/Handoff'

const STATUS: Record<Handoff['status'], { label: string; tone: string }> = {
  queued: { label: 'Queued', tone: 'neutral' },
  working: { label: 'Working', tone: 'info' },
  waiting: { label: 'Waiting', tone: 'warning' },
  done: { label: 'Done', tone: 'good' },
  failed: { label: 'Failed', tone: 'critical' },
  cancelled: { label: 'Taken back', tone: 'neutral' },
}

export function handoffLook(h: Handoff): { label: string; tone: string } {
  return h.kind === 'news' ? { label: 'Told', tone: 'neutral' } : STATUS[h.status]
}
