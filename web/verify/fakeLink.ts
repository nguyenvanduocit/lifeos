// A stand-in for src/core/link.ts (vite.config.ts swaps it in): the same exports, answered from made-up data. The
// Assistant is mid-turn: owner bubbles, a finished turn with its summary, a Start over rule, an error, a message from a
// routine and one to a crew member, a long run of steps, a thought, a group of tools (one failed), a message streaming in
// by deltas, and the owner's next message queued. A crew member, Coach, is at rest with a short stream.
import { ref, shallowReactive } from 'vue'
import type { Activity } from '../src/core/generated/Activity'
import type { Actor } from '../src/core/generated/Actor'
import type { AgentStanding } from '../src/core/generated/AgentStanding'
import type { AgentView } from '../src/core/generated/AgentView'
import type { Card } from '../src/core/generated/Card'
import type { Delivery } from '../src/core/generated/Delivery'
import type { EventDelta } from '../src/core/generated/EventDelta'
import type { EventKind } from '../src/core/generated/EventKind'
import type { EventRead } from '../src/core/generated/EventRead'
import type { ReadTool, ReadTools } from '../src/core/generated/tools'
import type { TurnMark } from '../src/core/generated/TurnMark'

export type { AgentView, Delivery, EventDelta, EventRead, ReadTool, ReadTools }
export const ASSISTANT = 'assistant'
export type WatchReply = { Changed: number } | { Event: EventRead } | { Delta: EventDelta } | { Agent: AgentView }
export const linkState = shallowReactive({ status: 'open' as 'connecting' | 'open' | 'down', error: '' })
export const revision = ref(0)

const params = new URLSearchParams(location.search)
const idle = params.get('as') === 'idle'
const still = params.get('still') === '1'
const calls: unknown[] = []
;(window as unknown as { calls: unknown[] }).calls = calls

const listeners = new Set<(r: WatchReply) => void>()
export const onWatch = (fn: (r: WatchReply) => void): void => void listeners.add(fn)
export const onReconnect = (_fn: () => void): void => {}
const push = (r: WatchReply) => listeners.forEach((fn) => fn(r))

const MIN = 60_000
const now = Date.now()
const iso = (ms: number) => new Date(ms).toISOString()
const OWNER: Actor = { kind: 'owner', id: '', name: 'You' }
const ROUTINE: Actor = { kind: 'routine', id: 'morning', name: 'Morning brief' }
const ASSISTANT_BY: Actor = { kind: 'assistant', id: ASSISTANT, name: 'Assistant' }

let nextId = 100
interface Seed {
  status?: string
  title?: string
  text?: string
  by?: Actor
  data?: unknown
  activity?: Activity | null
  ms?: number
  tool_name?: string
}
function ev(card: string, kind: EventKind, at: number, s: Seed = {}): EventRead {
  const id = nextId++
  return {
    event: {
      id, card, kind, status: s.status ?? '', tool_call_id: kind === 'tool' ? `tc_${id}` : '', tool_kind: '', tool_name: s.tool_name ?? '',
      title: s.title ?? '', text: s.text ?? '', by: s.by ?? null, data: s.data === undefined ? '' : JSON.stringify(s.data),
      created: iso(at), updated: iso(at + (s.ms ?? 0)), rev: 1,
    },
    activity: s.activity ?? null,
  }
}
const tool = (card: string, at: number, name: string, summary: string, group: 'lifeos' | 'read' | 'search' | 'fetch', status: 'completed' | 'failed' | 'running', input: unknown, output: string, ms = 900) =>
  ev(card, 'tool', at, { tool_name: `mcp__lifeos__${name}`, title: summary, ms, data: { input, output }, activity: { tool: { group, name, summary, status } }, status: status === 'running' ? 'in_progress' : status })

// The Assistant's stream.
const A = ASSISTANT
const t0 = now - 50 * MIN
const assistantEvents: EventRead[] = []
const add = (e: EventRead) => (assistantEvents.push(e), e)
add(ev(A, 'prompt', t0, { by: OWNER, status: 'delivered', text: "What's on my plate today?" }))
add(ev(A, 'thought', t0 + 2000, { text: 'The owner wants today at a glance: open tasks, habits not done, and anything from the journal. I will read each.' }))
add(tool(A, t0 + 4000, 'list_tasks', 'open tasks due today', 'lifeos', 'completed', { filter: 'today' }, '[{"id":"t_12","title":"Renew passport","due":"today"},{"id":"t_15","title":"Send invoice to Lan","due":"today"}]'))
add(tool(A, t0 + 5000, 'habits', 'habits for today', 'lifeos', 'completed', {}, '[{"habit":"Run","done_today":false},{"habit":"Read 20 pages","done_today":true}]'))
add(tool(A, t0 + 6000, 'read_journal', 'journal of yesterday', 'read', 'failed', { day: '2026-11-24' }, 'error: no journal entry for 2026-11-24'))
add(tool(A, t0 + 7000, 'money_month', 'spending this month', 'lifeos', 'completed', { month: '2026-11' }, '{"spent_minor":1843000,"budget_minor":2500000,"currency":"VND"}'))
add(ev(A, 'message', t0 + 9000, { text: "Here is today:\n\n- **Renew passport** (due today)\n- **Send invoice to Lan** (due today)\n- Habits: *Run* is still open; you already read today.\n\nYou have spent 18.4M of your 25M budget this month. I could not read yesterday's journal: there is no entry for it." }))
add(ev(A, 'stop', t0 + 10000, {
  status: 'end_turn',
  activity: { turn_end: { ms: 10_400, turn: { touched: [{ group: 'lifeos', name: 'list_tasks', summary: 'read tasks', count: 1 }, { group: 'lifeos', name: 'habits', summary: 'read habits', count: 1 }, { group: 'lifeos', name: 'money_month', summary: 'read money', count: 1 }], failures: [{ event_id: 104, kind: 'failed', name: 'read_journal', summary: 'no journal entry for 2026-11-24' }] } } },
}))
const t1 = now - 30 * MIN
add(ev(A, 'system', t1, { status: 'new_session', by: OWNER, title: 'Started over', text: 'The next message opens a new session, which reads a recap of this conversation.', activity: 'started_over' }))
add(ev(A, 'prompt', t1 + MIN, { by: ROUTINE, status: 'delivered', title: 'Write the morning brief', text: 'Write the morning brief: what is due, what was missed, one thing to look forward to. Save it with save_review (kind brief).' }))
add(tool(A, t1 + MIN + 3000, 'dashboard', 'the overview', 'lifeos', 'completed', {}, '{"tasks_due":2,"habits_open":1}'))
add(tool(A, t1 + MIN + 4000, 'save_review', 'saved the brief', 'lifeos', 'completed', { kind: 'brief' }, 'saved reviews/2026-11-25-brief.md'))
add(ev(A, 'message', t1 + MIN + 6000, { text: 'The brief for today is saved.' }))
add(ev(A, 'stop', t1 + MIN + 7000, { status: 'end_turn', activity: { turn_end: { ms: 6_900, turn: { touched: [{ group: 'lifeos', name: 'dashboard', summary: 'read the overview', count: 1 }, { group: 'lifeos', name: 'save_review', summary: 'saved the brief', count: 1 }], failures: [] } } } }))
const t2 = now - 12 * MIN
add(ev(A, 'prompt', t2, { by: OWNER, status: 'delivered', text: 'Ask Coach to plan three runs for this week.' }))
add(ev(A, 'tool', t2 + 3000, { tool_name: 'mcp__lifeos__message_agent', ms: 300, status: 'completed', activity: { sent: { to: 'coach', kind: 'request', label: 'Plan three runs for this week around the owner’s calendar', text: 'Plan three runs for this week. The owner runs in the morning and has meetings Tuesday and Thursday until 10.' } } }))
add(ev(A, 'message', t2 + 5000, { text: 'I asked Coach; its plan comes back here when it is done.', activity: { marked: { marks: [{ by: OWNER, mark: 'helpful', words: '', at: new Date(t2 + 9000).toISOString() }] } } }))
// A block (remit's ADR-0091): a choice of three with details, a picked `many` choice, a two-series line chart and a bar chart.
add(ev(A, 'system', t2 + 5500, {
  status: 'block',
  title: 'Runs this week',
  activity: {
    block: {
      block: {
        spec: {
          title: 'Runs this week',
          parts: [
            { choice: { id: 'when', label: 'When to run', many: false, picked: ['am'], options: [
              { id: 'am', label: 'Mornings, 6:30', detail: 'Before meetings; cooler, and the habit you kept last month' },
              { id: 'noon', label: 'Lunch, 12:15', detail: 'Shorter runs; Tuesday and Thursday only' },
              { id: 'pm', label: 'Evenings, 19:00', detail: 'Longest window, but dinner moves later' },
            ] } },
            { choice: { id: 'days', label: 'Days', many: true, picked: ['mon', 'wed'], options: [
              { id: 'mon', label: 'Monday', detail: null }, { id: 'wed', label: 'Wednesday', detail: null }, { id: 'sat', label: 'Saturday', detail: 'The long one' },
            ] } },
            { chart: { id: 'sleep', label: 'Sleep and runs', kind: 'line', unit: 'h', x: ['Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun'], series: [
              { name: 'Sleep', values: [7, 6.5, 7.5, 6, 8, 8.5, 7] }, { name: 'Run', values: [0.5, 0, 0.6, 0, 0, 1.2, 0] },
            ] } },
            { chart: { id: 'km', label: 'Kilometres a week', kind: 'bar', unit: 'km', x: ['W44', 'W45', 'W46', 'W47'], series: [{ name: 'km', values: [12, 15, 9, 18] }] } },
          ],
        },
        picked_by: null,
        picked_at: null,
      },
    },
  },
}))
add(ev(A, 'stop', t2 + 6000, { status: 'end_turn', activity: { turn_end: { ms: 5_800, turn: null } } }))
add(ev(A, 'system', t2 + 2 * MIN, { status: 'error', text: 'Claude Code exited with status 1: You have reached your usage limit. It resets at 18:00.' }))
// The turn in progress: a long run of steps, then the answer streaming.
const t3 = now - 3 * MIN
const asked = add(ev(A, 'prompt', t3, { by: OWNER, status: 'delivered', text: 'Log 7 hours of sleep for last night, and tell me how my sleep went this week.' }))
add(ev(A, 'thought', t3 + 2000, { text: 'Log the sleep first, then read the week of sleep metrics.' }))
add(tool(A, t3 + 4000, 'log_metric', 'sleep 7 h', 'lifeos', 'completed', { name: 'sleep', value: 7, unit: 'h' }, 'logged sleep 7 h for 2026-11-24'))
add(ev(A, 'thought', t3 + 6000, { text: 'Now the week.' }))
add(tool(A, t3 + 8000, 'metric_series', 'sleep, last 7 days', 'lifeos', 'completed', { name: 'sleep', days: 7 }, '[6.5,7,5.5,8,6,7,7]'))
add(ev(A, 'system', t3 + 9000, { status: 'retrying', text: 'The request was retried after a 529 from the API.' }))
add(tool(A, t3 + 11000, 'search', 'notes about sleep', 'search', 'completed', { query: 'sleep' }, '2 notes'))
add(ev(A, 'thought', t3 + 12000, { text: 'Compare with last week and look at the late nights.' }))
add(tool(A, t3 + 13000, 'read_doc', 'notes/sleep-plan.md', 'read', 'completed', { id: 'notes/sleep-plan.md' }, '# Sleep plan\nLights out by 23:00.'))
add(ev(A, 'message', t3 + 15000, { text: 'Logged **7 h** for last night.' }))
add(ev(A, 'thought', t3 + 16000, { text: 'Now the week summary: average, the worst night, and one suggestion.' }))
add(tool(A, t3 + 17000, 'metric_series', 'sleep, the 7 days before', 'lifeos', 'completed', { name: 'sleep', days: 14 }, '[7,7,6.5,7.5,7,6,8]'))
add(tool(A, t3 + 18000, 'list_goals', 'goals about health', 'lifeos', 'completed', {}, '[{"title":"Sleep 7h a night"}]'))
add(tool(A, t3 + 19000, 'read_journal', 'journal, Wednesday', 'read', 'failed', { day: '2026-11-19' }, 'error: no journal entry'))
add(tool(A, t3 + 20000, 'habits', 'habits this week', 'lifeos', 'running', {}, ''))
const SAY = 'This week you slept **6 h 43 min** a night on average, a little under your goal of 7 h.\n\n- Your shortest night was **Wednesday** (5 h 30 min).\n- Thursday was your best (8 h).\n\nOne suggestion: your sleep plan says lights out by 23:00; on the three short nights you logged screen time after midnight. Want me to add a reminder at 22:30?'
const streaming = add(ev(A, 'message', t3 + 24000, { text: idle ? SAY : SAY.slice(0, 120) }))
const queued = add(ev(A, 'prompt', now - 20_000, { by: OWNER, status: 'queued', text: 'And remind me to call mom on Sunday.' }))
if (idle) {
  assistantEvents.pop()
  assistantEvents.push(ev(A, 'stop', now - 10_000, { status: 'end_turn', activity: { turn_end: { ms: 170_000, turn: null } } }))
}

const card = (id: string, name: string, extra: Partial<Card>): Card => ({
  id, name, model: 'opus', effort: 'high', account: 'personal', session_id: `s-${id}-4f2a`, status: 'idle', activity: '', plan: [],
  ctx_used: 61_000, ctx_size: 200_000, turns: 9, queued: 0, output: '', error: '', cwd: '~/LifeOS', created: iso(now - 3 * 24 * 60 * MIN), updated: iso(now), rev: 1, ...extra,
})
const standing = (id: string, extra: Partial<AgentStanding>): AgentStanding => ({
  card: id, label: 'Idle', tone: 'idle', doing: 'Ready', doing_tone: 'idle', work: '', since: null, waiting: [], asks: ['message', 'restart'],
  remembers: true, on_message: null, on_send: null, stop_says: '', ...extra,
})
const views: Record<string, AgentView> = {
  [A]: idle
    ? { card: card(A, 'Assistant', { turns: 14 }), standing: standing(A, {}), draft: { card: A, text: '', images: [], rev: 0 } }
    : {
        card: card(A, 'Assistant', {
          status: 'working', activity: 'Answering', queued: 1, turns: 14, ctx_used: 98_000,
          plan: [
            { content: 'Log last night’s sleep', priority: 'high', status: 'completed' },
            { content: 'Read the week of sleep and compare with the week before', priority: 'medium', status: 'in_progress' },
            { content: 'Suggest one change, and offer a reminder', priority: 'low', status: 'pending' },
          ],
        }),
        standing: standing(A, {
          label: 'Working', tone: 'running', doing: 'Answering…', doing_tone: 'busy', work: 'Read the week of sleep', since: iso(t3),
          asks: ['message', 'steer', 'interrupt', 'restart', 'stop'], stop_says: 'Its turn ends and the messages waiting for it are dropped. Your why stays in its stream, and it reads it with your next message.', on_message: { event_id: asked.event.id, answering: true, at: iso(t3) },
          on_send: 'Waits for the current turn (+1 queued)',
        }),
        draft: { card: A, text: '', images: [], rev: 0 },
      },
  coach: { card: card('coach', 'Coach', { output: 'Three runs: Monday 7:00 easy 5 km, Wednesday 7:00 intervals, Saturday 8:00 long 10 km.' }), standing: standing('coach', { waiting: [7], label: 'Needs you', tone: 'needs', doing: 'Waiting on your answer', doing_tone: 'needs' }), draft: { card: 'coach', text: '', images: [], rev: 0 } },
}
const coachEvents: EventRead[] = [
  ev('coach', 'prompt', t2 + 3000, { by: ASSISTANT_BY, status: 'delivered', title: 'Plan three runs for this week', text: 'Plan three runs for this week. The owner runs in the morning and has meetings Tuesday and Thursday until 10.' }),
  ev('coach', 'message', t2 + 60_000, { text: 'Three runs: Monday 7:00 easy 5 km, Wednesday 7:00 intervals, Saturday 8:00 long 10 km.' }),
  ev('coach', 'stop', t2 + 61_000, { status: 'end_turn', activity: { turn_end: { ms: 58_000, turn: null } } }),
]
void queued

const CREW = [
  { id: 'coach', name: 'Coach', role: 'Running and sleep coach', instructions: '', model: null, effort: null, tools: [], connections: [], status: 'active', memories: 3 },
  { id: 'ledger', name: 'Ledger', role: 'Keeps the money in order', instructions: '', model: null, effort: null, tools: [], connections: [], status: 'active', memories: 0 },
]
const MEMORIES = [
  { id: 1, holder: A, kind: 'preference', title: 'Runs in the morning', body: 'Prefers to run before 8:00.', created: '2026-11-01 07:00', updated: '2026-11-01 07:00', ended: null, why_ended: null, recalled: 4 },
  { id: 2, holder: A, kind: 'fact', title: 'Budget is 25M VND a month', body: '', created: '2026-10-02 09:00', updated: '2026-10-02 09:00', ended: null, why_ended: null, recalled: 2 },
]

const QUESTIONS = [
  { id: 7, asker: 'coach', asker_name: 'Coach', text: 'Can I move Saturday’s long run to Sunday? The forecast says rain.', status: 'left', answer: '', why: 'It changes a plan you made yourself; only you can say.', decided_by: null, overturned: '', created: iso(now - 4 * MIN), decided: '' },
  { id: 6, asker: 'ledger', asker_name: 'Ledger', text: 'Should the 450k at Highlands count as Food or Work?', status: 'decided', answer: 'Food', why: 'You filed the last four coffee shop payments as Food.', decided_by: ASSISTANT_BY, overturned: '', created: iso(now - 40 * MIN), decided: iso(now - 39 * MIN) },
  { id: 5, asker: 'coach', asker_name: 'Coach', text: 'Five runs or three this week?', status: 'answered', answer: 'Three', why: '', decided_by: OWNER, overturned: 'Five', created: iso(now - 3 * 60 * MIN), decided: iso(now - 2 * 60 * MIN) },
  { id: 4, asker: 'ledger', asker_name: 'Ledger', text: 'Which budget should the gym fee go under?', status: 'open', answer: '', why: '', decided_by: null, overturned: '', created: iso(now - 30_000), decided: '' },
]
const wait = (ms = 150) => new Promise((r) => setTimeout(r, ms))
export async function read<T extends ReadTool>(tool: T, _input: ReadTools[T]['input']): Promise<ReadTools[T]['output']> {
  await wait()
  if (tool === 'list_crew') return CREW as ReadTools[T]['output']
  if (tool === 'list_memories') return MEMORIES as ReadTools[T]['output']
  if (tool === 'list_questions') return QUESTIONS as ReadTools[T]['output']
  throw new Error(`verify: no data for ${tool}`)
}
export async function agent(id: string): Promise<AgentView> {
  calls.push(['agent', id])
  await wait()
  return structuredClone(views[id] ?? { card: card(id, id, {}), standing: standing(id, {}), draft: { card: id, text: '', images: [], rev: 0 } })
}
export async function events(id: string, before?: number): Promise<EventRead[]> {
  calls.push(['events', id, before])
  await wait()
  return structuredClone(id === A ? assistantEvents : id === 'coach' ? coachEvents : [])
}
export async function say(id: string, text: string, delivery: Delivery): Promise<void> {
  calls.push(['say', id, text, delivery])
  await wait(400)
  const list = id === A ? assistantEvents : coachEvents
  const e = ev(id, 'prompt', Date.now(), { by: OWNER, status: views[id]?.card.status === 'working' ? 'queued' : 'delivered', text })
  list.push(e)
  push({ Event: structuredClone(e) })
}
export async function brief(): Promise<void> {
  calls.push(['brief'])
}
export async function interrupt(id: string): Promise<void> {
  calls.push(['interrupt', id])
}
export async function restart(id: string): Promise<void> {
  calls.push(['restart', id])
}
export async function draft(id: string, text: string): Promise<void> {
  calls.push(['draft', id, text])
}
export async function blockPick(event: number, part: string, picked: string[]): Promise<void> {
  calls.push(['blockPick', event, part, picked])
  await wait(400)
}
export async function overturn(question: number, word: string): Promise<void> {
  calls.push(['overturn', question, word])
}
export async function teach(event: number, mark: TurnMark, words: string): Promise<void> {
  calls.push(['teach', event, mark, words])
}
export async function stop(card: string, reason: string): Promise<void> {
  calls.push(['stop', card, reason])
}

// The answer streams in by deltas, a few words at a time, and the turn ends when it is all there.
export function start(): void {
  if (idle || still) return
  let at = streaming.event.text.length
  let chunks = 0
  const timer = setInterval(() => {
    if (at >= SAY.length) {
      clearInterval(timer)
      return
    }
    const text = SAY.slice(at, at + 14)
    push({ Delta: { id: streaming.event.id, card: A, kind: 'message', text, offset: at } })
    at += text.length
    // The server saves the growing message now and then: a page that missed a chunk catches up on the saved copy.
    if (++chunks % 4 === 0) {
      streaming.event.text = SAY.slice(0, at)
      streaming.event.rev++
      push({ Event: structuredClone(streaming) })
    }
  }, 350)
}
