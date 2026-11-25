// The page's one link to the server. The wasm module (lifeos-web-client) owns
// the WebTransport session and the postcard frames; this file only loads it,
// redials when the link drops, and types what it hands back with the types
// generated from Rust (lifeos-wire and the `tools!` list).
//
// The page only looks and talks: it runs `read` tools and chats with the
// agent. Nothing here can change a life.

import { ref, shallowReactive } from 'vue'
import init, { Link } from './pkg/web/lifeos_web_client.js'
import type { ChatDone } from './generated/ChatDone'
import type { LinkInfo } from './generated/LinkInfo'
import type { Progress } from './generated/Progress'
import type { Turn } from './generated/Turn'
import type { ReadTool, ReadTools } from './generated/tools'

export type { ChatDone, Progress, Turn, ReadTool, ReadTools }

export const linkState = shallowReactive({
  status: 'connecting' as 'connecting' | 'open' | 'down',
  error: '',
})

/** Grows whenever the data changes on the server; reads re-run on it. */
export const revision = ref(0)

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
  // Every (re)connect counts as a change: what was missed is read again.
  link.watch(() => {
    revision.value++
  }).catch(() => {})
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

export async function chat(message: string, onProgress: (p: Progress) => void): Promise<ChatDone> {
  return (await connect()).chat(message, onProgress) as Promise<ChatDone>
}

export async function brief(onProgress: (p: Progress) => void): Promise<ChatDone> {
  return (await connect()).brief(onProgress) as Promise<ChatDone>
}

export async function history(): Promise<Turn[]> {
  return (await connect()).history() as Promise<Turn[]>
}

export async function resetChat(): Promise<void> {
  await (await connect()).resetChat()
}

export function start(): void {
  void connect()
}
