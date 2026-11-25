// Turns a card's events into what the stream shows, as remit's stream.ts does: consecutive tool calls fold into one
// summary line ("read 3 files, called LifeOS 2 times"), thinking is its own block, and each turn ends in its row: how long
// it took and, as the server tells it, what it touched and what failed.
import type { Activity } from '../../core/generated/Activity'
import type { Actor } from '../../core/generated/Actor'
import type { Block as Shown } from '../../core/generated/Block'
import type { ActorKind } from '../../core/generated/ActorKind'
import type { EventRead } from '../../core/generated/EventRead'
import type { ImageRef } from '../../core/generated/ImageRef'
import type { OnMessage } from '../../core/generated/OnMessage'
import type { ToolGroup } from '../../core/generated/ToolGroup'
import type { TurnSummary } from '../../core/generated/TurnSummary'
import { plural, type Look, type Tone } from './look'

export type { ToolGroup }

type ToolActivity = Extract<Activity, { tool: unknown }>['tool']
type SentActivity = Extract<Activity, { sent: unknown }>['sent']

export type Block =
  | { type: 'prompt'; key: string; event: EventRead }
  | { type: 'message'; key: string; event: EventRead }
  | { type: 'thought'; key: string; event: EventRead }
  | { type: 'tools'; key: string; events: EventRead[]; summary: string; running: boolean; failed: number }
  | { type: 'turnEnd'; key: string; event: EventRead; ms?: number; turn?: TurnSummary }
  | { type: 'work'; key: string; event: EventRead } // a background task of the agent's process
  | { type: 'system'; key: string; event: EventRead }
  | { type: 'block'; key: string; event: EventRead; block: Shown } // what the agent showed with `show_block` (remit's ADR-0091), its picks as they stand
  | { type: 'stopped'; key: string; event: EventRead; reason: string; dropped: number } // the owner's stop, and why
  | { type: 'instructions'; key: string; events: EventRead[] } // instructions files the session read one after another: each titled by its path
  | ({ type: 'sent'; key: string; event: EventRead } & SentActivity)

// A tool call as the server typed it: its group, the name and words it is shown by, where it stands. An event the server
// sent untyped is shown as a tool of no group.
const UNTYPED: ToolActivity = { group: 'other', name: 'tool', summary: '', status: 'running' }
export function toolOf(e: EventRead): ToolActivity {
  const a = e.activity
  return a && typeof a === 'object' && 'tool' in a ? a.tool : { ...UNTYPED, name: e.event.tool_name || 'tool', summary: e.event.title }
}

// A memory a turn was given, as its prompt keeps it (the server's `PromptData.recalled`).
export interface Recalled {
  id: number
  holder: string // whose memory it is: the agent's own, or (for a crew member) the Assistant's
  kind: string
  title: string
}

// What an event's data holds that the page reads: a tool call's input and output, a stop's turn, the images an owner's
// message holds and the memories the turn on it was given. JSON text, '' for none.
export interface EventData {
  images?: ImageRef[]
  recalled?: Recalled[]
  input?: unknown
  output?: string
  tool?: string
  command?: string
}
export function dataOf(e: EventRead): EventData {
  if (!e.event.data) return {}
  try {
    const d = JSON.parse(e.event.data)
    return d && typeof d === 'object' ? (d as EventData) : {}
  } catch {
    return {}
  }
}

// A tool group's mark.
export const TOOL_ICON: Record<ToolGroup, string> = { read: 'file', edit: 'pencil', execute: 'terminal', search: 'search', fetch: 'download', lifeos: 'link', other: 'gear' }

const PHRASE: Record<ToolGroup, (n: number) => string> = {
  read: (n) => `read ${plural(n, 'file')}`,
  edit: (n) => `edited ${plural(n, 'file')}`,
  execute: (n) => `ran ${plural(n, 'command')}`,
  search: (n) => `searched ${plural(n, 'time')}`,
  fetch: (n) => `fetched ${plural(n, 'page')}`,
  lifeos: (n) => `called LifeOS ${plural(n, 'time')}`,
  other: (n) => `used ${plural(n, 'other tool')}`,
}
const ORDER: ToolGroup[] = ['edit', 'execute', 'read', 'search', 'fetch', 'lifeos', 'other']

// The words for a group of tool calls, counted by the group the server gave each.
export function toolSummary(events: EventRead[]): string {
  const counts = new Map<ToolGroup, number>()
  for (const e of events) counts.set(toolOf(e).group, (counts.get(toolOf(e).group) ?? 0) + 1)
  return ORDER.flatMap((g) => {
    const n = counts.get(g)
    return n ? [PHRASE[g](n)] : []
  }).join(', ')
}

const isRunning = (e: EventRead) => toolOf(e).status === 'running'
const isFailed = (e: EventRead) => toolOf(e).status === 'failed'

// blocks expects one card's events, oldest first.
export function blocks(events: readonly EventRead[]): Block[] {
  const out: Block[] = []
  let tools: EventRead[] = []
  const flush = () => {
    if (!tools.length) return
    out.push({ type: 'tools', key: 'g' + tools[0].event.id, events: tools, summary: toolSummary(tools), running: tools.some(isRunning), failed: tools.filter(isFailed).length })
    tools = []
  }
  for (const e of events) {
    const a = e.activity
    if (e.event.kind === 'tool') {
      if (!(a && typeof a === 'object' && 'sent' in a)) {
        tools.push(e)
        continue
      }
      flush()
      out.push({ type: 'sent', key: 's' + e.event.id, event: e, ...a.sent })
      continue
    }
    flush()
    const key = String(e.event.id)
    switch (e.event.kind) {
      case 'prompt':
        out.push({ type: 'prompt', key, event: e })
        break
      case 'message':
        out.push({ type: 'message', key, event: e })
        break
      case 'thought':
        out.push({ type: 'thought', key, event: e })
        break
      case 'task':
        out.push({ type: 'work', key, event: e })
        break
      case 'stop': {
        const end = a && typeof a === 'object' && 'turn_end' in a ? a.turn_end : undefined
        out.push({ type: 'turnEnd', key, event: e, ms: end?.ms ?? undefined, turn: end?.turn ?? undefined })
        break
      }
      default:
        if (a && typeof a === 'object' && 'block' in a) out.push({ type: 'block', key, event: e, block: a.block.block })
        else if (a && typeof a === 'object' && 'stopped' in a) out.push({ type: 'stopped', key, event: e, ...a.stopped })
        else if (e.event.status === 'instructions_read') {
          const last = out.at(-1)
          if (last?.type === 'instructions') last.events.push(e)
          else out.push({ type: 'instructions', key, events: [e] })
        } else out.push({ type: 'system', key, event: e })
    }
  }
  flush()
  return out
}

// Only what the owner sent, himself or through the Assistant (its message to a crew member), is his words in the stream
// (a chat bubble), by who sent it (every event carries its actor).
export const ownerSent = (e: EventRead): boolean => e.event.by?.kind === 'owner' || e.event.by?.kind === 'assistant'

// The agent that sent a prompt: its id; undefined when the owner, the Assistant for him, or LifeOS sent it.
export const agentSender = (e: EventRead): string | undefined => (e.event.by?.kind === 'agent' ? e.event.by.id : undefined)

// A prompt another agent sent, as against the news of the agent's own background work (its own card is the sender).
export const fromAgent = (e: EventRead): boolean => agentSender(e) !== undefined && agentSender(e) !== e.event.card

// A row of the agent working its way to what it says next (a tool call, a thought, a note), as against what it says, what
// the owner sent or must see (a block it showed, his stop), a message between it and another agent, and where a turn or
// session ends.
export function isStep(b: Block): boolean {
  switch (b.type) {
    case 'tools':
    case 'thought':
    case 'work':
    case 'instructions':
      return true
    case 'prompt':
      return !ownerSent(b.event) && !fromAgent(b.event)
    case 'system':
      return b.event.event.status !== 'error' && b.event.event.status !== 'new_session'
    default:
      return false
  }
}

// More rows of steps than this between two messages fold into one line the owner opens.
const FOLD_OVER = 5
export interface StepRun {
  key: string
  start: number
  end: number
}
// foldedRuns finds the runs of steps that a message closes and that are long enough to fold. The run after the last
// message stays open: the agent is still at it.
export function foldedRuns(bs: readonly Block[]): StepRun[] {
  const out: StepRun[] = []
  let start = -1
  bs.forEach((b, i) => {
    if (isStep(b)) {
      if (start < 0) start = i
      return
    }
    if (b.type === 'message' && start >= 0 && i - start > FOLD_OVER) out.push({ key: 'f' + bs[start].key, start, end: i })
    start = -1
  })
  return out
}

// Where one step of an agent's work stands, as its mark shows it.
const STEP: Record<string, Look & { label: string }> = {
  completed: { tone: 'success', icon: 'check', label: 'Done' },
  failed: { tone: 'danger', icon: 'alert', label: 'Failed' },
  stopped: { tone: 'quiet', icon: 'stop', label: 'Stopped' },
}
export function stepLook(status: string | undefined): Look & { label: string; working: boolean } {
  const ended = STEP[status ?? '']
  return ended ? { ...ended, working: false } : { tone: 'info', label: 'Running', working: true }
}

// A tool call as its card shows it: the name and words the server gave it, and how long it took.
export function toolCall(e: EventRead, now = Date.now()): { name: string; summary: string; ms: number; running: boolean } {
  const { name, summary } = toolOf(e)
  const running = isRunning(e)
  const ms = (running ? now : new Date(e.event.updated).getTime()) - new Date(e.event.created).getTime()
  return { name, summary, ms, running }
}

// A system event that is not an error is a note in the stream; its icon and tone say which, its text the rest.
export function noteOf(e: EventRead): Required<Look> {
  const { status } = e.event
  if (status === 'refused') return { icon: 'x', tone: 'danger' }
  if (status === 'retrying' || status === 'model_switched') return { icon: 'repeat', tone: 'warning' }
  if (status === 'compacted') return { icon: 'archive', tone: 'quiet' }
  if (status === 'waiting') return { icon: 'clock', tone: 'quiet' }
  return { icon: 'interrupt', tone: 'quiet' }
}

// Who did something, as the owner reads it.
const ACTOR: Record<ActorKind, (by: Actor, nameOf: (id: string) => string) => string> = {
  owner: () => 'You',
  assistant: () => 'Assistant · for you',
  agent: (by, nameOf) => by.name || (by.id ? nameOf(by.id) : 'An agent'),
  lifeos: () => 'LifeOS',
  routine: (by) => (by.name ? `Routine · ${by.name}` : 'A routine'),
  workflow: (by) => (by.name ? `Workflow · ${by.name}` : 'A workflow'),
}
export const actorName = (by: Actor, nameOf: (id: string) => string): string => ACTOR[by.kind](by, nameOf)

// Where a message put to an agent stands: has it arrived, is it being read, was it read.
export interface Receipt {
  text: string
  tone: Tone
  queued: boolean
}
export function receipt(e: EventRead, onMessage: OnMessage | null | undefined): Receipt {
  if (e.event.status === 'queued') return { text: 'Sent · waiting for the current work to finish', tone: 'warning', queued: true }
  if (e.event.status === 'dropped') return { text: 'Not read — the session closed first', tone: 'danger', queued: false }
  if (onMessage?.event_id === e.event.id) return { text: onMessage.answering ? 'Processing…' : 'Reading…', tone: 'info', queued: false }
  return { text: 'Read', tone: 'success', queued: false }
}
