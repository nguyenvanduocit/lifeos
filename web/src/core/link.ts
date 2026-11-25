// The page's one link to the server. The wasm module (lifeos-web-client) owns
// the WebTransport session and the postcard frames; this file only loads it,
// redials when the link drops, and types what it hands back with the types
// generated from Rust (lifeos-wire and the `tools!` list).
//
// The page only looks and talks: it runs `read` tools and chats with the
// agent. Nothing here can change a life; what it writes is the owner's own
// acts on the agents (a message, a pick in a block, an overturn, a mark on a
// reply, a stop).

import { ref, shallowReactive } from 'vue'
import init, { Link } from './pkg/web/lifeos_web_client.js'
import type { AgentView } from './generated/AgentView'
import type { CardContext } from './generated/CardContext'
import type { CardTelemetry } from './generated/CardTelemetry'
import type { Delivery } from './generated/Delivery'
import type { EventDelta } from './generated/EventDelta'
import type { EventRead } from './generated/EventRead'
import type { Image } from './generated/Image'
import type { ImageRef } from './generated/ImageRef'
import type { ImageType } from './generated/ImageType'
import type { LinkInfo } from './generated/LinkInfo'
import type { ReadTool, ReadTools } from './generated/tools'
import type { TurnMark } from './generated/TurnMark'

export type { AgentView, CardContext, CardTelemetry, Delivery, EventDelta, EventRead, Image, ImageRef, ImageType, ReadTool, ReadTools, TurnMark }

/** Who the Assistant is, among the agents a page can talk to (a card id is its agent's id). */
export const ASSISTANT = 'assistant'

/** One reply of the watch: data changed, or an agent's stream or standing moved. */
export type WatchReply = { Changed: number } | { Event: EventRead } | { Delta: EventDelta } | { Agent: AgentView }

export const linkState = shallowReactive({
  status: 'connecting' as 'connecting' | 'open' | 'down',
  error: '',
})

/** Grows whenever the data changes on the server; reads re-run on it. */
export const revision = ref(0)

/** Who hears the agents' replies of the watch (the Assistant's store), and each (re)connect. */
const listeners = new Set<(r: WatchReply) => void>()
const reconnects = new Set<() => void>()
export function onWatch(fn: (r: WatchReply) => void): void {
  listeners.add(fn)
}
export function onReconnect(fn: () => void): void {
  reconnects.add(fn)
}

const REDIAL_MS = 2000
let current: Promise<Link> | null = null

async function dial(): Promise<Link> {
  linkState.status = 'connecting'
  await init()
  const res = await fetch('/link.json')
  if (!res.ok) throw new Error(`/link.json: ${res.status}`)
  const info = (await res.json()) as LinkInfo
  const link = await Link.connect(info)
  linkState.status = 'open'
  linkState.error = ''
  link.closed().then((why: string) => {
    linkState.status = 'down'
    linkState.error = why
    current = null
    // Redial at once, so the page keeps watching.
    setTimeout(() => void connect(), REDIAL_MS)
  })
  // One watch per link: `Changed` re-runs the pages' reads (every (re)connect counts as one, so what was missed is read
  // again); the agents' events, deltas and standings go to whoever listens.
  link.watch((reply: WatchReply) => {
    if ('Changed' in reply) revision.value++
    else for (const fn of listeners) fn(reply)
  }).catch(() => {})
  for (const fn of reconnects) queueMicrotask(fn)
  return link
}

function connect(): Promise<Link> {
  if (!current) {
    current = dial().catch(async (err: unknown) => {
      linkState.status = 'down'
      linkState.error = String(err)
      current = null
      await new Promise((r) => setTimeout(r, REDIAL_MS))
      return connect()
    })
  }
  return current
}

export async function read<T extends ReadTool>(tool: T, input: ReadTools[T]['input']): Promise<ReadTools[T]['output']> {
  return (await connect()).read(tool, input) as Promise<ReadTools[T]['output']>
}

/** Agent `card` as its drawer shows it. */
export async function agent(card: string): Promise<AgentView> {
  return (await connect()).agent(card) as Promise<AgentView>
}

/** The newest events of `card`'s stream, or those before event `before`, oldest first. */
export async function events(card: string, before?: number): Promise<EventRead[]> {
  return (await connect()).events(card, before ?? null) as Promise<EventRead[]>
}

/** What fills agent `card`'s context and what it was given, counted afresh while its session runs. */
export async function context(card: string): Promise<CardContext> {
  return (await connect()).context(card) as Promise<CardContext>
}

/** What agent `card`'s model requests and tools did, as its sessions exported it. */
export async function telemetry(card: string): Promise<CardTelemetry> {
  return (await connect()).telemetry(card) as Promise<CardTelemetry>
}

/** Sends the owner's message with the images put for it; resolves once it is queued (the agent's work arrives on the
 * watch). */
export async function say(card: string, text: string, images: ImageRef[], delivery: Delivery): Promise<void> {
  await (await connect()).say(card, text, images, delivery)
}

/** Asks the Assistant to write today's morning brief. */
export async function brief(): Promise<void> {
  await (await connect()).brief()
}

/** Ends the turn `card` is in. */
export async function interrupt(card: string): Promise<void> {
  await (await connect()).interrupt(card)
}

/** Start over: `card`'s next message opens a new session. */
export async function restart(card: string): Promise<void> {
  await (await connect()).restart(card)
}

/** Keeps what the owner is typing to `card` on the server, and the images put for it. */
export async function draft(card: string, text: string, images: ImageRef[]): Promise<void> {
  await (await connect()).draft(card, text, images)
}

/** Keeps an image's bytes as `id` (an id the page makes), for a draft to name; an id put again keeps its first bytes. */
export async function imagePut(id: string, image: Image): Promise<void> {
  await (await connect()).image_put(id, image)
}

/** The image kept as `id`. */
export async function imageGet(id: string): Promise<Image> {
  return (await connect()).image_get(id) as Promise<Image>
}

/** The owner picks `picked` (option ids) in the choice `part` of block event `event`; resolves once the agent's message
 * is queued. */
export async function blockPick(event: number, part: string, picked: string[]): Promise<void> {
  await (await connect()).block_pick(event, part, picked)
}

/** The owner overturns the answer given to question `question` with `word`, or answers one left to him. */
export async function overturn(question: number, word: string): Promise<void> {
  await (await connect()).overturn(question, word)
}

/** The owner marks agent reply `event`: helpful, not helpful (`words` why) or correct (`words` the correct reply). */
export async function teach(event: number, mark: TurnMark, words: string): Promise<void> {
  await (await connect()).teach(event, mark, words)
}

/** The owner stops agent `card`: its turn ends, what waits for it is dropped, and `reason` stays in its stream. */
export async function stop(card: string, reason: string): Promise<void> {
  await (await connect()).stop(card, reason)
}

export function start(): void {
  void connect()
}
