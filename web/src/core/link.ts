// The page's one link to the server. The wasm module (lifeos-web-client) owns
// the WebTransport session and the postcard frames; this file only loads it,
// redials when the link drops, and types what it hands back with the types
// ts-rs generated from lifeos-wire.

import { shallowReactive } from 'vue'
import init, { Link } from './pkg/web/lifeos_web_client.js'
import type { ChatDone } from './generated/ChatDone'
import type { LinkInfo } from './generated/LinkInfo'
import type { Progress } from './generated/Progress'
import type { Today } from './generated/Today'

export type { ChatDone, Progress, Today }

export const linkState = shallowReactive({
  status: 'connecting' as 'connecting' | 'open' | 'down',
  error: '',
})

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
  })
  return link
}

function link(): Promise<Link> {
  if (!current) {
    current = dial().catch(async (err: unknown) => {
      linkState.status = 'down'
      linkState.error = String(err)
      current = null
      await new Promise((r) => setTimeout(r, REDIAL_MS))
      return link()
    })
  }
  return current
}

export async function today(): Promise<Today> {
  return (await link()).today() as Promise<Today>
}

export async function chat(message: string, onProgress: (p: Progress) => void): Promise<ChatDone> {
  return (await link()).chat(message, onProgress) as Promise<ChatDone>
}

export async function resetChat(): Promise<void> {
  await (await link()).resetChat()
}
