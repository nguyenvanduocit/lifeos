<script setup lang="ts">
// Team talk (docs/adr/0002): everything the agents asked and handed each other, as threads. A thread starts with what
// one agent sent while working for nobody else; what the agent that had it handed on for it sits under it, so a chain
// (Assistant → Fin → Coach) reads top to bottom with each answer beside its question. Nothing here acts on the agents:
// the owner opens their chats, or drafts a word to the Assistant (a hand-off to take back, a question about the work).
import { computed, reactive, watch } from 'vue'
import { AlertTriangle, Bot, CircleCheck, ListFilter, MessagesSquare, Radio, Search } from '@lucide/vue'
import { useRoute } from 'vue-router'
import { ASSISTANT } from '../assistant'
import AskChip from '../components/AskChip.vue'
import LoadState from '../components/LoadState.vue'
import PageHeader from '../components/ds/PageHeader.vue'
import StatStrip from '../components/ds/StatStrip.vue'
import Segmented from '../components/ds/Segmented.vue'
import HandoffItem from '../components/agents/HandoffItem.vue'
import type { Handoff } from '../core/generated/Handoff'
import { useRead } from '../data'
import { useQueryRef } from '../nav'

const { data, error, loading, retry } = useRead('list_handoffs', () => ({ open: false }))
const { data: crew } = useRead('list_crew')

const show = useQueryRef('show', 'all', ['all', 'open', 'failed', 'news'] as const)
const agent = useQueryRef<string>('agent', '')
const agents = computed(() => [
  { id: ASSISTANT, name: 'Assistant' },
  ...(crew.value ?? []).map((m) => ({ id: m.id, name: m.name })),
])

const rows = computed<readonly Handoff[]>(() => data.value ?? [])
const byId = computed(() => new Map(rows.value.map((h) => [h.id, h])))
// What each hand-off had handed on for it, oldest first.
const kids = computed(() => {
  const m = new Map<number, Handoff[]>()
  for (const h of rows.value) {
    if (h.parent != null && byId.value.has(h.parent)) m.set(h.parent, [...(m.get(h.parent) ?? []), h])
  }
  for (const list of m.values()) list.sort((a, b) => a.id - b.id)
  return m
})
// The threads, newest first: hand-offs sent for no other (or whose parent is older than the list goes back).
const roots = computed(() => rows.value.filter((h) => h.parent == null || !byId.value.has(h.parent)))

const isOpen = (h: Handoff) => h.status === 'queued' || h.status === 'working' || h.status === 'waiting'
// Every hand-off of a thread.
function thread(h: Handoff): Handoff[] {
  return [h, ...(kids.value.get(h.id) ?? []).flatMap(thread)]
}
const matches = (h: Handoff) => {
  if (agent.value && h.sender !== agent.value && h.target !== agent.value) return false
  if (show.value === 'open') return isOpen(h)
  if (show.value === 'failed') return h.status === 'failed'
  if (show.value === 'news') return h.kind === 'news'
  return true
}
// A thread shows whole when any of it matches.
const shown = computed(() => roots.value.filter((r) => thread(r).some(matches)))

const day = 24 * 60 * 60 * 1000
const recent = (h: Handoff) => Date.now() - Date.parse(h.closed || h.created) < day
const out = computed(() => rows.value.filter(isOpen).length)
const done = computed(() => rows.value.filter((h) => h.status === 'done' && h.kind !== 'news' && recent(h)).length)
const failed = computed(() => rows.value.filter((h) => h.status === 'failed' && recent(h)).length)
const quiet = computed(() => rows.value.filter((h) => isOpen(h) && h.note).length)

// Opened hand-offs; `?h=` opens one and the thread it is in.
const open = reactive(new Set<number>())
const toggle = (id: number) => (open.has(id) ? open.delete(id) : open.add(id))
const route = useRoute()
watch(
  () => [route.query.h, data.value] as const,
  ([h]) => {
    const id = Number(h)
    if (!Number.isInteger(id) || !byId.value.has(id)) return
    for (let at = byId.value.get(id); at; at = at.parent != null ? byId.value.get(at.parent) : undefined) open.add(at.id)
    queueMicrotask(() => document.querySelector(`[data-handoff="${id}"]`)?.scrollIntoView({ block: 'center', behavior: 'smooth' }))
  },
  { immediate: true },
)

const filters = computed(() => [
  { value: 'all' as const, label: 'All', count: roots.value.length },
  { value: 'open' as const, label: 'Still out', count: out.value },
  { value: 'failed' as const, label: 'Failed', count: rows.value.filter((h) => h.status === 'failed').length, urgent: true },
  { value: 'news' as const, label: 'News', count: rows.value.filter((h) => h.kind === 'news').length },
])
const strip = computed(() => [
  { key: 'out', icon: Radio, value: String(out.value), label: quiet.value ? `still out · ${quiet.value} quiet` : 'still out' },
  { key: 'done', icon: CircleCheck, value: String(done.value), label: 'done today' },
  { key: 'failed', icon: AlertTriangle, value: String(failed.value), label: 'failed today', urgent: failed.value > 0 },
])
</script>

<template>
  <PageHeader title="Team talk" sub="What your agents asked and handed each other, as threads: who has what, how it stands, and what came back.">
    <AskChip size="sm" primary :icon="Search" words="How is the work you handed out going? Read team_status and tell me what is still out and what failed." label="Ask how it's going" />
    <template v-if="rows.length" #strip><StatStrip :items="strip" /></template>
  </PageHeader>

  <LoadState :loading="loading && !data" :error="error" label="team talk" class="mb-4" @retry="retry" />
  <template v-if="data">
    <section class="min-w-0 overflow-hidden rounded-2xl border border-line bg-panel">
      <div class="flex flex-wrap items-center gap-3 px-5 py-3.5" :class="rows.length ? 'border-b border-line' : ''">
        <Segmented v-model="show" :options="filters" label="Show" />
        <label class="flex items-center gap-2 text-[13px] text-ink2">
          <Bot class="size-4 text-ink3" aria-hidden="true" /><span class="sr-only">Filter by agent</span>
          <select v-model="agent" class="lf-select h-8 max-w-48 cursor-pointer appearance-none rounded-[10px] border border-line2 bg-panel pr-7 pl-2.5 text-[13px] text-ink hover:bg-panel2">
            <option value="">All agents</option>
            <option v-for="a in agents" :key="a.id" :value="a.id">{{ a.name }}</option>
          </select>
        </label>
        <span class="ml-auto text-[13px] text-ink3 tabular-nums">{{ shown.length }} thread{{ shown.length === 1 ? '' : 's' }}</span>
      </div>
      <div v-if="!rows.length" class="flex flex-col items-center gap-3 px-5 py-14 text-center">
        <span class="grid size-11 place-items-center rounded-xl bg-panel2"><MessagesSquare class="size-5 text-ink2" /></span>
        <p class="text-[15px] font-medium">No talk between agents yet</p>
        <p class="max-w-md text-[13.5px] text-ink2">When an agent asks another (ask_agent) or hands it work (hand_off), it shows here: who has what, how it stands, and what came back.</p>
      </div>
      <div v-else-if="!shown.length" class="flex flex-col items-center gap-3 px-5 py-12 text-center">
        <span class="grid size-11 place-items-center rounded-xl bg-panel2"><ListFilter class="size-5 text-ink2" /></span>
        <p class="text-[15px] font-medium">Nothing matches these filters.</p>
      </div>
      <ul v-else class="space-y-3 p-4">
        <HandoffItem v-for="h in shown" :key="h.id" :handoff="h" :kids="kids" :open="open" @toggle="toggle" />
      </ul>
    </section>
  </template>
</template>

<style scoped>
.lf-select {
  background-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='%23888' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'%3E%3Cpath d='m6 9 6 6 6-6'/%3E%3C/svg%3E");
  background-size: 14px;
  background-position: right 8px center;
  background-repeat: no-repeat;
}
</style>
