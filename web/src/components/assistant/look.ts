// Small display helpers the panel's components share, as remit's ui.ts has them (the part the panel uses).
import type { AgentTone } from '../../core/generated/AgentTone'
import type { DoingTone } from '../../core/generated/DoingTone'
import { ASSISTANT } from '../../core/link'

// An agent as a lettered square: the Assistant in its violet, any other agent in a colour from its id.
export function agentLetter(name: string): string {
  return name.trim().slice(0, 2).replace(/^./, (c) => c.toUpperCase()) || '?'
}
export function agentStyle(id: string): Record<string, string> {
  if (id === ASSISTANT) return { background: 'var(--violet-bg)', color: 'var(--violet-text)' }
  let h = 0
  for (const ch of id) h = (h * 31 + ch.charCodeAt(0)) % 360
  return { background: `hsl(${h} 35% 18%)`, color: `hsl(${h} 70% 72%)` }
}

export const tokens = (n: number): string => (n >= 1e6 ? (n / 1e6).toFixed(1) + 'M' : n >= 1e3 ? Math.round(n / 1e3) + 'k' : String(n || 0))

export function hm(iso: string | number): string {
  return new Date(iso).toLocaleTimeString('en-GB', { hour: '2-digit', minute: '2-digit' })
}

// The whole date and time.
export function fullTime(iso: string): string {
  return new Date(iso).toLocaleString('en-GB')
}

export function ago(iso: string, now = Date.now()): string {
  const s = (now - new Date(iso).getTime()) / 1000
  if (s < 60) return 'just now'
  if (s < 3600) return Math.floor(s / 60) + ' min ago'
  if (s < 86400) return Math.floor(s / 3600) + ' h ago'
  return Math.floor(s / 86400) + ' d ago'
}

// An agent's Markdown as one plain line of at most n characters.
export function short(s: string | undefined, n: number): string {
  if (!s) return ''
  const one = s
    .replace(/\[([^\]]+)\]\([^)]*\)/g, '$1')
    .replace(/^\s{0,3}(#{1,6}|[-*+]|\d+\.)\s+/gm, '')
    .replace(/(\*\*|__)(?=\S)(.+?)(?<=\S)\1/g, '$2')
    .replace(/(?<![\w*])([*_])(?=\S)([^*_]+?)(?<=\S)\1(?![\w*])/g, '$2')
    .replace(/`+([^`]+)`+/g, '$1')
    .replace(/\s+/g, ' ')
    .trim()
  return one.length > n ? one.slice(0, n - 1) + '…' : one
}

// A count with its noun: 1 turn, 3 turns.
export const plural = (n: number, noun: string, many = noun + 's'): string => `${n} ${n === 1 ? noun : many}`

// A length of time in its largest units: 27s, 4m, 1h 12m.
export function dur(ms: number): string {
  const s = Math.max(0, Math.round(ms / 1000))
  if (s < 60) return s + 's'
  const m = Math.floor(s / 60)
  if (m < 60) return m + 'm'
  return Math.floor(m / 60) + 'h ' + (m % 60) + 'm'
}

// How long a step took, to the second while it is under an hour: 42s, 1m 23s, 1h 5m.
export function durSec(ms: number): string {
  const s = Math.max(0, Math.round(ms / 1000))
  return s >= 60 && s < 3600 ? Math.floor(s / 60) + 'm ' + (s % 60) + 's' : dur(ms)
}

// How a state reads at a glance: its tone (a Badge variant and a text colour) and, beside its words, its icon.
export type Tone = 'quiet' | 'info' | 'success' | 'warning' | 'gate' | 'needs' | 'danger'
export interface Look {
  tone: Tone
  icon?: string
}
export const toneText: Record<Tone, string> = {
  quiet: 'text-mut',
  info: 'text-info',
  success: 'text-success',
  warning: 'text-warning',
  gate: 'text-sand',
  needs: 'text-orange',
  danger: 'text-destructive',
}

// The icon of an agent's badge, by the tone the server gave it.
const GLYPH: Record<string, string> = { failed: 'alert', needs: 'clock', waiting: 'clock', done: 'check' }
export const badgeGlyph = (tone: string): string => GLYPH[tone] ?? ''

const AGENT_TONE: Record<AgentTone, Tone> = { needs: 'needs', waiting: 'gate', failed: 'danger', running: 'info', done: 'success', idle: 'quiet' }
export const agentTone = (tone: AgentTone): Tone => AGENT_TONE[tone]

// The colour of the line saying what an agent is doing.
const DOING: Partial<Record<DoingTone, string>> = { needs: toneText.needs, waiting: toneText.gate, error: toneText.danger }
export const doingText = (doingTone: DoingTone): string => DOING[doingTone] ?? ''

// Where an MCP server an agent was given stands, as Claude Code reports it; a status it does not name shows as itself.
const MCP_LOOK: Record<string, Look & { label: string }> = {
  connected: { tone: 'success', icon: 'check', label: 'Connected' },
  failed: { tone: 'danger', icon: 'alert', label: 'Failed' },
  'needs-auth': { tone: 'needs', icon: 'key', label: 'Needs sign-in' },
  pending: { tone: 'warning', icon: 'clock', label: 'Pending' },
}
export const mcpLook = (status: string): Look & { label: string } => MCP_LOOK[status] ?? { tone: 'quiet', label: status }

// The Claude Code tools a session holds, in words: none given is every tool.
export const allowedText = (tools: readonly string[] | null): string => (tools === null ? 'every tool' : tools.length ? tools.join(', ') : 'none')

// A time the server wrote as `YYYY-MM-DD HH:MM` (local), as every browser reads it: an ISO time is left as it is.
export const at = (s: string): string => (/^\d{4}-\d{2}-\d{2} \d/.test(s) ? s.replace(' ', 'T') : s)
