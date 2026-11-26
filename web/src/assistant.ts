// The agents beside every page: the Assistant and each crew member, each one card (its id the agent's), shown in the
// panel as remit shows its Assistant (docs/adr/0001). Pages never change a life's data; they put words in an agent's box
// (`say`) for the owner to finish and send, or send them right away (`ask`). The panel writes only the owner's own acts
// on the agents: his messages, picks in blocks, overturns, marks on replies and stops (core/link). Nothing is decided here: the server sends each
// card's standing (`AgentView`) and its stream (events, then deltas that grow a message or a thought), and this store
// keeps the newest copy of each.
import { reactive, ref } from 'vue'
import * as link from './core/link'
import { putAttachment } from './images'
import { ASSISTANT, type AgentView, type Delivery, type EventRead, type ImageRef, type WatchReply } from './core/link'

export { ASSISTANT }

/** How many events one read of a stream brings back at most (the server's page). */
const PAGE = 400

/** A message of the owner's the server has not taken yet: its words and images, and what is happening to them. */
export interface Pending {
  id: number
  text: string
  images: ImageRef[]
  delivery: Delivery
  status: 'sending' | 'slow' | 'failed'
  why: string
}

export interface AgentState {
  id: string
  /** Its standing, its card and the server's copy of its draft; null until first read. */
  view: AgentView | null
  /** Its stream, oldest first, one copy per event id (the higher `rev` kept). */
  events: EventRead[]
  /** Whether the stream was read (it is read when the panel first shows the agent). */
  loaded: boolean
  /** Whether the server may hold events older than the first one here. */
  older: boolean
  error: string
  /** What is in the box, as the owner types it; the server keeps a copy (`link.draft`). */
  draft: string
  /** The images in the box, put on the server already; the draft names them. */
  images: ImageRef[]
  /** How many attachments are being put on the server for the box. */
  putting: number
  /** What could not be attached, in words for the owner. */
  notAdded: string
  pending: Pending[]
}

const agents = reactive<Record<string, AgentState>>({})
// The panel's open state lives in the address (?assistant=1, App.vue), as remit's does; who it talks to is kept here.
export const panel = reactive({
  /** Who the panel talks to: `assistant` or a crew member id. */
  agent: localStorage.getItem('lifeos.agent') ?? ASSISTANT,
  /** Grows when the box should take the focus (a page put words in it). */
  focusTick: 0,
})
/** Whether the panel is open, as the address says; App.vue keeps the two in step. */
export const panelOpen = ref(false)
/** The address's hand on the panel: App.vue sets it, so `setOpen` moves the address. */
let openHook: (open: boolean) => void = (open) => (panelOpen.value = open)
export function onOpen(hook: (open: boolean) => void): void {
  openHook = hook
}

function blank(id: string): AgentState {
  return { id, view: null, events: [], loaded: false, older: false, error: '', draft: '', images: [], putting: 0, notAdded: '', pending: [] }
}

/** The agent `id` as the store holds it; its standing is read the first time it is asked for. */
export function agentState(id: string): AgentState {
  if (!agents[id]) {
    agents[id] = blank(id)
    void loadView(id)
  }
  return agents[id]
}

async function loadView(id: string): Promise<void> {
  try {
    adopt(await link.agent(id))
  } catch (err) {
    agentState(id).error = messageOf(err)
  }
}

/** Reads the newest of `id`'s stream (and its standing), replacing what was held: when the panel first shows it, and
 * after the link came back, when what was missed is unknown. */
export async function loadStream(id: string): Promise<void> {
  const a = agentState(id)
  try {
    const [view, events] = await Promise.all([link.agent(id), link.events(id)])
    adopt(view)
    a.events = events
    a.older = hasOlder(events)
    a.loaded = true
    a.error = ''
  } catch (err) {
    a.error = messageOf(err)
  }
}

/** Whether the server may hold events before `page`: it was full, or it begins at a Start over (the server cuts a page
 * there, and what came before is the page before). */
function hasOlder(page: EventRead[]): boolean {
  return page.length >= PAGE || page[0]?.activity === 'started_over'
}

/** Reads the page of `id`'s stream before the oldest event held. */
export async function loadOlder(id: string): Promise<void> {
  const a = agentState(id)
  const first = a.events[0]
  if (!first) return
  try {
    const older = await link.events(id, first.event.id)
    const have = new Set(a.events.map((e) => e.event.id))
    a.events = [...older.filter((e) => !have.has(e.event.id)), ...a.events]
    a.older = hasOlder(older)
  } catch (err) {
    a.error = messageOf(err)
  }
}

// The server's draft is taken into the box when it is newer than the last the box agreed with, and the owner has not
// typed since: another tab's words, or the box after a reload.
const draftRev: Record<string, number> = {}
const typing: Record<string, boolean> = {}
function adopt(view: AgentView): void {
  const id = view.card.id
  const a = agents[id] ?? (agents[id] = blank(id))
  a.view = view
  if (view.draft.rev > (draftRev[id] ?? -1)) {
    draftRev[id] = view.draft.rev
    if (!typing[id]) {
      a.draft = view.draft.text
      a.images = view.draft.images
    }
  }
}

/** Puts an event in its place in the stream, keeping the copy with the higher `rev`. */
function merge(a: AgentState, e: EventRead): void {
  const list = a.events
  for (let i = list.length - 1; i >= 0; i--) {
    const have = list[i].event
    if (have.id === e.event.id) {
      if (e.event.rev >= have.rev) list[i] = e
      return
    }
    if (have.id < e.event.id) {
      list.splice(i + 1, 0, e)
      return
    }
  }
  list.unshift(e)
}

function hear(reply: WatchReply): void {
  if ('Agent' in reply) return adopt(reply.Agent)
  if ('Event' in reply) {
    const a = agents[reply.Event.event.card]
    if (!a?.loaded) return
    // A Start over begins the conversation anew, as the server's page of it does: what came before is the page before.
    if (reply.Event.activity === 'started_over') {
      a.events = [reply.Event]
      a.older = true
    } else merge(a, reply.Event)
    return
  }
  if ('Delta' in reply) {
    const d = reply.Delta
    const a = agents[d.card]
    if (!a?.loaded) return
    for (let i = a.events.length - 1; i >= 0; i--) {
      const e = a.events[i].event
      if (e.id !== d.id) continue
      // A chunk lands where its offset says; one whose start is past what is held waits for the event's next copy.
      if (e.text.length >= d.offset) e.text = e.text.slice(0, d.offset) + d.text
      return
    }
  }
}

link.onWatch(hear)
link.onReconnect(() => {
  for (const [id, a] of Object.entries(agents)) {
    if (a.loaded) void loadStream(id)
    else void loadView(id)
  }
})

/** Reads the Assistant's standing at once, so pages know whether it is at work. */
export function start(): void {
  agentState(ASSISTANT)
}

// The box: what is typed goes to the server a moment after the owner stops typing, with the images in it.
const DRAFT_MS = 400
const timers: Record<string, ReturnType<typeof setTimeout>> = {}
export function setDraft(id: string, text: string): void {
  agentState(id).draft = text
  keepSoon(id)
}
/** The attachments in the box have already been uploaded. */
export function setImages(id: string, images: ImageRef[]): void {
  agentState(id).images = images
  keepSoon(id)
}
/** Puts files on the server and adds them to the box's attachments, one by one as each is there. */
export async function attach(id: string, files: readonly File[]): Promise<void> {
  if (!files.length) return
  const a = agentState(id)
  a.putting += files.length
  a.notAdded = ''
  for (const file of files) {
    try {
      const attachment = await putAttachment(file)
      setImages(id, [...a.images, attachment])
    } catch (e) {
      a.notAdded += `${a.notAdded ? ' ' : ''}Could not attach ${file.name}. ${messageOf(e)}`
    } finally {
      a.putting--
    }
  }
}
function keepSoon(id: string): void {
  typing[id] = true
  clearTimeout(timers[id])
  timers[id] = setTimeout(() => void keepDraft(id), DRAFT_MS)
}
async function keepDraft(id: string): Promise<void> {
  const a = agentState(id)
  const { draft: text, images } = a
  try {
    await link.draft(id, text, images)
  } catch {
    // The box keeps the words; the server's copy is only a convenience.
  } finally {
    if (a.draft === text && a.images === images) typing[id] = false
  }
}

let pendingIds = 0
const SLOW_MS = 800
/** Sends the box's words as `delivery`: they leave the box at once for a row at the end of the stream, which says how
 * they go, until the server's copy of the message arrives. */
export function send(id: string, delivery: Delivery): void {
  const a = agentState(id)
  const text = a.draft.trim()
  const images = a.images
  if (!text && !images.length) return
  a.draft = ''
  a.images = []
  clearTimeout(timers[id])
  typing[id] = false
  void link.draft(id, '', []).catch(() => {})
  const p: Pending = { id: ++pendingIds, text, images, delivery, status: 'sending', why: '' }
  a.pending.push(p)
  void deliver(a, p)
}

async function deliver(a: AgentState, p: Pending): Promise<void> {
  const slow = setTimeout(() => (find(a, p.id).status = 'slow'), SLOW_MS)
  try {
    await link.say(a.id, p.text, p.images, p.delivery)
    a.pending = a.pending.filter((x) => x.id !== p.id)
  } catch (err) {
    const row = find(a, p.id)
    row.status = 'failed'
    row.why = messageOf(err)
  } finally {
    clearTimeout(slow)
  }
}
const find = (a: AgentState, id: number): Pending => a.pending.find((x) => x.id === id) ?? ({} as Pending)

/** A failed send, tried again. */
export function resend(id: string, pendingId: number): void {
  const a = agentState(id)
  const p = find(a, pendingId)
  if (!p.text && !p.images?.length) return
  p.status = 'sending'
  p.why = ''
  void deliver(a, p)
}
/** A failed send, its words and images taken back into the box. */
export function unsend(id: string, pendingId: number): void {
  const a = agentState(id)
  const p = find(a, pendingId)
  a.pending = a.pending.filter((x) => x.id !== pendingId)
  if (p.images?.length) setImages(id, [...a.images, ...p.images])
  if (p.text) setDraft(id, a.draft.trim() ? `${a.draft}\n${p.text}` : p.text)
}

export async function interrupt(id = panel.agent): Promise<void> {
  try {
    await link.interrupt(id)
  } catch (err) {
    agentState(id).error = messageOf(err)
  }
}

/** Start over: the agent's next message opens a new session. */
export async function restart(id = panel.agent): Promise<void> {
  try {
    await link.restart(id)
  } catch (err) {
    agentState(id).error = messageOf(err)
  }
}

export function setOpen(open: boolean): void {
  openHook(open)
}

export function talkTo(id: string): void {
  panel.agent = id
  localStorage.setItem('lifeos.agent', id)
  setOpen(true)
  panel.focusTick++
}

/** Puts words in an agent's box for the owner to finish, and opens it there. */
export function say(words: string, id = ASSISTANT): void {
  setDraft(id, words)
  talkTo(id)
}

/** What a drag carries that a drop on a page gives the agent: files, else links. */
export function carriesAttachable(types: readonly string[]): boolean {
  return types.includes('Files') || types.includes('text/uri-list')
}
/** The links a drop carries: text/uri-list's lines (those starting with # are comments), else text/plain when it is one
 * address. */
export function droppedLinks(data: DataTransfer): string[] {
  const listed = data.getData('text/uri-list').split(/\r?\n/).map((l) => l.trim()).filter((l) => l && !l.startsWith('#'))
  if (listed.length) return listed
  const plain = data.getData('text/plain').trim()
  return /^https?:\/\/\S+$/.test(plain) ? [plain] : []
}
/** Files or links dropped on a page: the panel opens on the agent it talks to, with the files attached to its box and the
 * links added to its words (the owner's message names them, as a link pasted in the box does). */
export function attachDropped(data: DataTransfer): void {
  const id = panel.agent
  const files = Array.from(data.files)
  const links = files.length ? [] : droppedLinks(data)
  if (!files.length && !links.length) return
  if (links.length) {
    const words = agentState(id).draft.trimEnd()
    setDraft(id, `${words ? `${words}\n` : ''}${links.join('\n')}\n`)
  }
  talkTo(id)
  void attach(id, files)
}

/** Sends a message now (to the Assistant unless told otherwise), shown in the panel. */
export function ask(message: string, id = ASSISTANT): Promise<void> {
  const text = message.trim()
  if (!text) return Promise.resolve()
  if (panel.agent !== id) talkTo(id)
  else setOpen(true)
  const a = agentState(id)
  const p: Pending = { id: ++pendingIds, text, images: [], delivery: 'queue', status: 'sending', why: '' }
  a.pending.push(p)
  return deliver(a, p)
}

/** Whether the brief was asked for and the Assistant has not finished the turn yet. */
export const briefing = ref(false)
export async function writeBrief(): Promise<void> {
  talkTo(ASSISTANT)
  briefing.value = true
  try {
    await link.brief()
  } catch (err) {
    agentState(ASSISTANT).error = messageOf(err)
    briefing.value = false
  }
}

/** Whether an agent is at work (a turn in progress, or starting one). */
export function busy(id = ASSISTANT): boolean {
  const status = agentState(id).view?.card.status
  return status === 'working' || status === 'starting'
}

// The brief is done when the Assistant goes back to rest after it was asked for.
link.onWatch((r) => {
  if (briefing.value && 'Agent' in r && r.Agent.card.id === ASSISTANT && r.Agent.card.status === 'idle' && !r.Agent.card.queued) briefing.value = false
})

export function messageOf(err: unknown): string {
  return err instanceof Error ? err.message : String(err)
}
