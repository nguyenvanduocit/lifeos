<script setup lang="ts">
// Everything LifeOS's agents remember, read-only (the librarian writes it, from what the owner says). Filters live in
// the address (?agent=, ?kind=, ?q=, ?forgotten=1). To forget or correct, the owner tells the agent; this page follows.
import { computed } from 'vue'
import { refDebounced } from '@vueuse/core'
import { Brain, Repeat, Search, ShieldCheck, Sparkles, ThumbsDown, Plus, Bot } from '@lucide/vue'
import { useRead } from '../data'
import { useQueryRef } from '../nav'
import { ASSISTANT } from '../assistant'
import AskChip from '../components/AskChip.vue'
import LoadState from '../components/LoadState.vue'
import PageHeader from '../components/ds/PageHeader.vue'
import StatStrip from '../components/ds/StatStrip.vue'
import Panel from '../components/ds/Panel.vue'
import Segmented from '../components/ds/Segmented.vue'
import MemoryRows from '../components/crew/MemoryRows.vue'
import { KINDS } from '../components/crew/kinds'

const { data: crew } = useRead('list_crew')
const names = computed<Record<string, string>>(() => Object.fromEntries([[ASSISTANT, 'Assistant'], ...(crew.value ?? []).map((m) => [m.id, m.name])]))

const agent = useQueryRef<string>('agent', '')
const kind = useQueryRef<string>('kind', 'all')
const q = useQueryRef<string>('q', '')
const forgotten = useQueryRef<'' | '1'>('forgotten', '', ['', '1'])
const query = refDebounced(q, 200)

const { data, error, loading, retry } = useRead('list_memories', () => ({
  holder: agent.value || undefined,
  query: query.value.trim() || undefined,
  include_forgotten: forgotten.value === '1',
}))
const all = computed(() => data.value ?? [])
const shown = computed(() => (kind.value === 'all' ? all.value : all.value.filter((m) => m.kind === kind.value)))
const kinds = computed(() => [
  { value: 'all', label: 'All', count: all.value.length },
  ...Object.entries(KINDS)
    .map(([value, k]) => ({ value, label: k.label + 's', count: all.value.filter((m) => m.kind === value).length }))
    .filter((k) => k.count),
])

const live = computed(() => all.value.filter((m) => !m.ended))
const strip = computed(() => [
  { key: 'kept', icon: Brain, value: String(live.value.length), label: agent.value ? `kept by ${names.value[agent.value] ?? agent.value}` : 'memories kept' },
  { key: 'confirmed', icon: ShieldCheck, value: String(live.value.filter((m) => m.trust === 'confirmed').length), label: 'you confirmed' },
  { key: 'recalled', icon: Repeat, value: String(live.value.reduce((n, m) => n + m.recalled, 0)), label: 'times recalled' },
  { key: 'misled', icon: ThumbsDown, value: String(live.value.filter((m) => m.misled > m.helped).length), label: 'misled more than helped', urgent: live.value.some((m) => m.misled > m.helped) },
])
</script>

<template>
  <PageHeader title="Memory" sub="What your agents remember across conversations. The librarian writes it from what you say; agents only read it.">
    <AskChip size="sm" :icon="Sparkles" words="Review my memories and list duplicates or outdated entries for my approval before merging or forgetting anything." label="Review memories" />
    <AskChip size="sm" primary :icon="Plus" words="Remember that " label="Add a memory" />
    <template v-if="data" #strip><StatStrip :items="strip" /></template>
  </PageHeader>

  <Panel title="Memories" :icon="Brain" :count="shown.length">
    <template #actions>
      <label class="flex h-8 w-full items-center gap-2 rounded-[10px] sm:w-56 border border-line2 bg-panel px-2.5 focus-within:outline-2 focus-within:outline-ring">
        <Search class="size-3.5 shrink-0 text-ink3" aria-hidden="true" />
        <input v-model="q" aria-label="Search memories" placeholder="Search memories…" class="min-w-0 flex-1 bg-transparent text-[13px] outline-none placeholder:text-ink3" />
      </label>
      <label class="flex items-center gap-2 text-[13px] text-ink2">
        <Bot class="size-4 text-ink3" aria-hidden="true" /><span class="sr-only">Agent</span>
        <select v-model="agent" class="lf-select h-8 max-w-44 cursor-pointer appearance-none rounded-[10px] border border-line2 bg-panel pr-7 pl-2.5 text-[13px] text-ink hover:bg-panel2">
          <option value="">All agents</option>
          <option v-for="(n, id) in names" :key="id" :value="id">{{ n }}</option>
        </select>
      </label>
    </template>
    <div class="flex flex-wrap items-center gap-3 border-t border-line px-5 py-3">
      <Segmented v-model="kind" :options="kinds" label="Kind" />
      <label class="ml-auto flex items-center gap-2 text-[13px] text-ink2">
        <input type="checkbox" class="size-3.5 accent-[var(--ink)]" :checked="forgotten === '1'" @change="forgotten = ($event.target as HTMLInputElement).checked ? '1' : ''" />
        Include forgotten
      </label>
    </div>
    <LoadState :loading="loading && !data" :error="error" label="memories" class="px-5 pb-4" @retry="retry" />
    <div v-if="data" class="border-t border-line">
      <MemoryRows v-if="shown.length" :memories="shown" :names="names" :show-holder="!agent" />
      <div v-else class="flex flex-col items-center gap-3 px-5 py-14 text-center">
        <span class="grid size-11 place-items-center rounded-xl bg-panel2"><Brain class="size-5 text-ink2" /></span>
        <p class="text-[15px] font-medium">{{ q.trim() ? 'No memory matches these words.' : 'Nothing remembered yet.' }}</p>
        <p class="max-w-md text-[13.5px] text-ink2">Tell an agent what to keep (a preference, a decision, how you like things done) and the librarian writes it down.</p>
      </div>
    </div>
  </Panel>
</template>

<style scoped>
.lf-select {
  background-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='%23888' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'%3E%3Cpath d='m6 9 6 6 6-6'/%3E%3C/svg%3E");
  background-size: 14px;
  background-position: right 8px center;
  background-repeat: no-repeat;
}
</style>
