// The owner's Vault (docs/data.md, "The Vault"): read and changed over the owner's own HTTP route, never the link, so no
// agent tool carries it. A value goes to the server in the request that saves it and comes back in nothing.
import { ref, shallowRef, watch } from 'vue'
import { revision } from './core/link'
import type { VaultView } from './core/generated/VaultView'
import type { VaultItem } from './core/generated/VaultItem'
import type { VaultKind } from './core/generated/VaultKind'
import type { SaveVaultItem } from './core/generated/SaveVaultItem'
import type { ImportVaultKeys } from './core/generated/ImportVaultKeys'
import { ASSISTANT } from './core/link'

async function call<T>(payload?: object, secrets: string[] = []): Promise<T> {
  const response = await fetch('/api/vault', {
    credentials: 'same-origin',
    cache: 'no-store',
    headers: { 'X-LifeOS-Account': '1', ...(payload ? { 'Content-Type': 'application/json' } : {}) },
    ...(payload ? { method: 'POST', body: JSON.stringify(payload) } : {}),
  })
  if (!response.headers.get('content-type')?.includes('application/json')) throw new Error('Could not reach the Vault.')
  const result = await response.json()
  if (!response.ok) {
    let message: string = result.error ?? 'The Vault said no.'
    for (const s of secrets) if (s) message = message.replaceAll(s, '…')
    throw new Error(message)
  }
  return result as T
}

/** The Vault, read again whenever the server says something changed. */
export function useVault() {
  const data = shallowRef<VaultView>()
  const error = ref('')
  const loading = ref(true)
  const attempt = ref(0)
  let seq = 0
  watch(
    [revision, attempt],
    async () => {
      const mine = ++seq
      loading.value = true
      try {
        const view = await call<VaultView>()
        if (mine === seq) {
          data.value = view
          error.value = ''
        }
      } catch (err) {
        if (mine === seq) error.value = err instanceof Error ? err.message : String(err)
      } finally {
        if (mine === seq) loading.value = false
      }
    },
    { immediate: true },
  )
  return { data, error, loading, retry: () => { attempt.value++ } }
}

export const saveVaultItem = (item: SaveVaultItem) =>
  call<{ ok: true; id: number }>({ action: 'save', item }, item.secret ? [item.secret] : [])
export const removeVaultItem = (id: number) => call<{ ok: true }>({ action: 'remove', id })
export const importVaultKeys = (keys: ImportVaultKeys) =>
  call<{ ok: true; imported: { added: number; replaced: number; kept: number } }>(
    { action: 'import', keys },
    keys.keys.map((k) => k.value),
  )

export const KIND_LABEL: Record<VaultKind, string> = { login: 'Login', key: 'Key', contact: 'Contact' }
export const KIND_TEXT: Record<VaultKind, string> = {
  login: "A website's username and password",
  key: 'A key your connections send, by its name',
  contact: 'Details agents type in forms',
}

/** One line on what an item is, never its value. */
export function itemLine(item: VaultItem): string {
  switch (item.kind) {
    case 'login':
      return [item.website?.replace(/^https?:\/\//, ''), item.username].filter(Boolean).join(' · ')
    case 'key':
      return [item.key_name !== item.title ? item.key_name : '', item.key_hint ? `••••${item.key_hint}` : 'no value'].filter(Boolean).join(' · ')
    case 'contact':
      return [item.contact?.name, item.contact?.company, item.contact?.email].filter(Boolean).join(' · ')
  }
}

/** Who may use an item, in words. */
export function whoLine(item: Pick<VaultItem, 'all_agents' | 'agents'>, names: Map<string, string>): string {
  if (item.all_agents) return 'Every agent'
  if (!item.agents.length) return 'No agent yet'
  return item.agents.map((a) => (a === ASSISTANT ? 'Assistant' : names.get(a) ?? a)).join(', ')
}

/**
 * A `.env` file's keys, read here in the browser (agentes.chat ADR 0144 §11): `NAME=value`, `export NAME=…`, comments,
 * quotes (double with `\n` `\t` `\"` `\\`, single or backticks as written), a quoted value over several lines, CRLF,
 * a byte order mark; or a JSON object of names and values. A name written twice keeps its last value.
 */
export function parseEnv(text: string): { keys: { name: string; value: string }[]; twice: string[] } {
  const src = text.replace(/^﻿/, '').replace(/\r\n?/g, '\n')
  const out = new Map<string, string>()
  const twice = new Set<string>()
  const put = (name: string, value: string) => {
    if (out.has(name)) twice.add(name)
    out.set(name, value)
  }
  const trimmed = src.trim()
  if (trimmed.startsWith('{')) {
    try {
      const obj = JSON.parse(trimmed) as Record<string, unknown>
      for (const [k, v] of Object.entries(obj)) {
        if (typeof v === 'string' || typeof v === 'number' || typeof v === 'boolean') put(k, String(v))
      }
      return { keys: [...out].map(([name, value]) => ({ name, value })), twice: [...twice] }
    } catch { /* not JSON: read it as lines */ }
  }
  let i = 0
  while (i < src.length) {
    const end = src.indexOf('\n', i)
    const line = src.slice(i, end === -1 ? src.length : end)
    const m = /^\s*(?:export\s+)?([A-Za-z_][A-Za-z0-9_.-]*)\s*=\s*(.*)$/.exec(line)
    if (!m) { i = end === -1 ? src.length : end + 1; continue }
    const name = m[1]
    let rest = m[2]
    const q = rest[0]
    if (q === '"' || q === "'" || q === '`') {
      // The value may run past this line: read from the opening quote in the whole text.
      const start = i + line.indexOf(rest) + 1
      let j = start
      let value = ''
      while (j < src.length && src[j] !== q) {
        if (q === '"' && src[j] === '\\' && j + 1 < src.length) {
          const n = src[j + 1]
          value += n === 'n' ? '\n' : n === 't' ? '\t' : n
          j += 2
        } else {
          value += src[j++]
        }
      }
      put(name, value)
      const after = src.indexOf('\n', j)
      i = after === -1 ? src.length : after + 1
      continue
    }
    rest = rest.replace(/\s+#.*$/, '').trim()
    put(name, rest)
    i = end === -1 ? src.length : end + 1
  }
  return { keys: [...out].map(([name, value]) => ({ name, value })), twice: [...twice] }
}

/** Why a key from a file can't be saved, or nothing. */
export function keyProblem(name: string, value: string): string {
  if (!/^[A-Z][A-Z0-9_]*$/.test(name)) return 'Not an UPPER_SNAKE name'
  if (['PATH', 'HOME', 'USER', 'SHELL', 'TMPDIR', 'LANG', 'PWD', 'ANTHROPIC_API_KEY', 'ANTHROPIC_AUTH_TOKEN', 'CLAUDE_CODE_OAUTH_TOKEN'].includes(name))
    return 'A name LifeOS keeps for itself'
  if (!value) return 'No value'
  if (/[\r\n\0]/.test(value)) return 'Spans several lines'
  if (value.length > 16384) return 'Over 16 KB'
  return ''
}
