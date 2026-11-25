// An Inbox item's text read for its parts: where it came from (a leading emoji, a [Source] tag, "Idea:",
// a link…), a title line, the rest, a suggested next step, and ids that are noise to a reader (gmail id).
import type { Component } from 'vue'
import { BookOpen, Lightbulb, Link, Mail, StickyNote } from '@lucide/vue'

export type Source = 'mail' | 'idea' | 'read' | 'link' | 'note'

export const SOURCES: Record<Source, { label: string; icon: Component; tone: string }> = {
  mail: { label: 'Email', icon: Mail, tone: 'inbox' },
  idea: { label: 'Idea', icon: Lightbulb, tone: 'goals' },
  read: { label: 'Read later', icon: BookOpen, tone: 'journal' },
  link: { label: 'Link', icon: Link, tone: 'integrations' },
  note: { label: 'Note', icon: StickyNote, tone: 'notes' },
}

export interface Parsed {
  source: Source
  /** The tag or sender, e.g. GitHub, Quang. */
  from?: string
  title: string
  body: string
  next?: string
  meta: string[]
}

const MAIL_EMOJI = /^(📧|✉️?|📨|📩|📬|📥)/u
const LEAD_EMOJI = /^\s*((?:\p{Extended_Pictographic}|\p{Emoji_Presentation})️?\s*)+/u

export function parseInbox(raw: string): Parsed {
  let text = raw.trim()
  let source: Source | undefined
  let from: string | undefined
  const meta: string[] = []

  const emoji = text.match(LEAD_EMOJI)
  if (emoji) {
    if (MAIL_EMOJI.test(emoji[0].trim())) source = 'mail'
    text = text.slice(emoji[0].length)
  }
  // Ids in parentheses (gmail id: …, message id: …) go to the meta line.
  text = text.replace(/\s*\(((?:gmail|message|mail|thread)\s*id:\s*[^)]+)\)\s*/giu, (_, id: string) => {
    meta.push(id.trim())
    if (/gmail|mail/i.test(id)) source ??= 'mail'
    return ' '
  }).trim()

  const tag = text.match(/^\[([^\]]{1,32})\]\s*/u)
  if (tag) {
    from = tag[1]
    text = text.slice(tag[0].length)
  }
  const lead = text.match(/^(idea|read later|email from ([^:]{1,40})|note|link)\s*:\s*/iu)
  if (lead) {
    const word = lead[1].toLowerCase()
    if (word === 'idea') source ??= 'idea'
    else if (word === 'read later') source ??= 'read'
    else if (word === 'link') source ??= 'link'
    else if (word.startsWith('email from')) {
      source ??= 'mail'
      from ??= lead[2].trim()
    } else source ??= 'note'
    text = text.slice(lead[0].length)
  }
  if (!source && /https?:\/\/\S+/u.test(text)) source = 'link'
  if (!source && /\b(gmail|email|e-mail)\b/iu.test(from ?? '')) source = 'mail'

  // A suggested step ("Nên làm: …", "Next: …", "Suggested: …") stands apart.
  let next: string | undefined
  const step = text.match(/\s*(?:nên làm|next step|next|suggested|todo)\s*:\s*(.+)$/iu)
  if (step && step.index! > 0) {
    next = step[1].trim().replace(/\.$/u, '')
    text = text.slice(0, step.index).trim()
  }

  // Title: up to an em dash, else the first sentence, when it is short enough to read as one.
  let title = text
  let body = ''
  const dash = text.search(/\s[—–]\s/u)
  const sentence = text.search(/[.!?]\s/u)
  const cut = dash > 0 && dash <= 160 ? dash : sentence > 0 && sentence <= 160 ? sentence + 1 : -1
  if (cut > 0) {
    title = text.slice(0, cut).trim()
    body = text.slice(cut).replace(/^\s*[—–]\s*/u, '').trim()
  }
  title = title.replace(/^./u, (c) => c.toUpperCase())
  if (body) body = body.replace(/^./u, (c) => c.toUpperCase())
  return { source: source ?? 'note', from, title, body, next, meta }
}

/** "created" as a Date (RFC 3339, or the older "YYYY-MM-DD HH:MM:SS"). */
export function createdAt(created: string): Date {
  return new Date(created.includes('T') ? created : created.replace(' ', 'T'))
}

const rel = new Intl.RelativeTimeFormat('en-US', { numeric: 'auto', style: 'short' })
export function ago(date: Date, now = new Date()): string {
  const s = Math.round((date.getTime() - now.getTime()) / 1000)
  const a = Math.abs(s)
  if (a < 60) return 'just now'
  if (a < 3600) return rel.format(Math.round(s / 60), 'minute')
  if (a < 86400) return rel.format(Math.round(s / 3600), 'hour')
  if (a < 86400 * 30) return rel.format(Math.round(s / 86400), 'day')
  return rel.format(Math.round(s / (86400 * 30)), 'month')
}
