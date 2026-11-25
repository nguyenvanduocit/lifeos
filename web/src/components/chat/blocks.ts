// From an agent's steps to what the chat draws (as remit's stream.ts does):
// words, a thinking fold, or a group of consecutive tool calls with their
// outcomes. Steps arrive in order; an outcome finds its call by id.
import type { Step } from '../../core/generated/Step'
import type { ToolCall } from '../../core/generated/ToolCall'
import type { ToolOutcome } from '../../core/generated/ToolOutcome'
import { toolWords } from '../../assistant'

export interface Call {
  call: ToolCall
  outcome?: ToolOutcome
}

export type Block =
  | { kind: 'text'; text: string }
  | { kind: 'thinking'; text: string }
  | { kind: 'tools'; calls: Call[] }

export function blocks(steps: readonly Step[]): Block[] {
  const out: Block[] = []
  const byId = new Map<string, Call>()
  for (const step of steps) {
    const last = out[out.length - 1]
    if ('Text' in step) {
      if (last?.kind === 'text') last.text += step.Text
      else out.push({ kind: 'text', text: step.Text })
    } else if ('Thinking' in step) {
      out.push({ kind: 'thinking', text: step.Thinking })
    } else if ('Tool' in step) {
      const call: Call = { call: step.Tool }
      byId.set(step.Tool.id, call)
      if (last?.kind === 'tools') last.calls.push(call)
      else out.push({ kind: 'tools', calls: [call] })
    } else {
      const call = byId.get(step.ToolDone.id)
      if (call) call.outcome = step.ToolDone
    }
  }
  return out
}

/** "thêm việc, ghi thói quen ×2" */
export function summary(calls: readonly Call[]): string {
  const counts = new Map<string, number>()
  for (const c of calls) {
    const w = toolWords(c.call.name)
    counts.set(w, (counts.get(w) ?? 0) + 1)
  }
  return [...counts].map(([w, n]) => (n > 1 ? `${w} ×${n}` : w)).join(', ')
}

export function failed(calls: readonly Call[]): number {
  return calls.filter((c) => c.outcome?.failed).length
}

/** 1,2 giây · 3 phút 5 giây */
export function duration(ms: number): string {
  const s = Math.round(ms / 1000)
  if (s < 60) return `${Math.max(1, s)} giây`
  const m = Math.floor(s / 60)
  return s % 60 ? `${m} phút ${s % 60} giây` : `${m} phút`
}

/** Pretty JSON when it parses, the text as is otherwise. */
export function pretty(text: string): string {
  try {
    return JSON.stringify(JSON.parse(text), null, 2)
  } catch {
    return text
  }
}
