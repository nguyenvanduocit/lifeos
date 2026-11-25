// How routines and their runs stand, in a word, a tone and a mark (the automation page and an agent's page).
import type { Component } from 'vue'
import { CalendarCheck, CircleCheck, CircleDashed, Clock, Hand, LoaderCircle, Pause, Webhook, XCircle, Zap } from '@lucide/vue'
import type { Routine } from '../../core/generated/Routine'
import type { Run } from '../../core/generated/Run'
import { cronWords, when } from '../agents/when'

export interface Look { label: string; tone: string; icon?: Component; pulse?: boolean; spin?: boolean }

export function routineLook(r: Routine): Look {
  if (r.status === 'paused') return { label: 'Paused', tone: 'warning', icon: Pause }
  if (r.status === 'done') return { label: 'Ran once', tone: 'neutral', icon: CalendarCheck }
  if (r.hook && !r.hook_ready) return { label: r.cron || r.at || r.on.length ? 'Enabled · webhook not ready' : 'Webhook not ready', tone: 'warning' }
  return { label: 'Enabled', tone: 'good', pulse: true }
}

export function runLook(run: Run): Look {
  if (run.finished) return run.error ? { label: 'Error', tone: 'critical', icon: XCircle } : { label: 'Completed', tone: 'good', icon: CircleCheck }
  return run.started ? { label: 'Running', tone: 'info', icon: LoaderCircle, spin: true, pulse: true } : { label: 'Queued', tone: 'neutral', icon: CircleDashed }
}

export const TRIGGERS: Record<string, { label: string; icon: Component }> = {
  schedule: { label: 'Scheduled', icon: Clock },
  hook: { label: 'Webhook', icon: Webhook },
  event: { label: 'Event', icon: Zap },
  manual: { label: 'Manual', icon: Hand },
}

// The ways a routine starts, each a chip.
export function triggers(r: Routine) {
  const out: { icon: Component; text: string; title?: string; code?: boolean }[] = []
  if (r.cron) {
    const words = cronWords(r.cron)
    out.push(words ? { icon: Clock, text: words, title: `cron ${r.cron}` } : { icon: Clock, text: r.cron, title: 'Cron expression (local time)', code: true })
  }
  if (r.at) out.push({ icon: CalendarCheck, text: `Once · ${when(r.at)}`, title: r.at })
  if (r.on.length) out.push({ icon: Zap, text: `After ${r.on.map((t) => t.replaceAll('_', ' ')).join(', ')}`, title: r.on.join(', ') })
  if (r.hook) out.push({ icon: Webhook, text: r.hook_ready ? 'Webhook' : 'Webhook · key required' })
  return out
}

export function lead(r: Routine): Component {
  if (r.cron) return Clock
  if (r.at) return CalendarCheck
  if (r.on.length) return Zap
  if (r.hook) return Webhook
  return Hand
}

export const WORKFLOW_STATUS: Record<string, { label: string; tone: string; pulse?: boolean }> = {
  running: { label: 'Running', tone: 'info', pulse: true },
  waiting: { label: 'Waiting', tone: 'warning' },
  done: { label: 'Done', tone: 'good' },
  failed: { label: 'Failed', tone: 'critical' },
  stopped: { label: 'Stopped', tone: 'neutral' },
}
