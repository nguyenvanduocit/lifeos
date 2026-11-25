<script setup lang="ts">
import LoadState from '../LoadState.vue'
// What an agent remembers, read-only. Forgetting is asked of the agent.
import { computed, ref } from 'vue'
import { Search } from '@lucide/vue'
import type { Memory } from '../../core/generated/Memory'
import { useRead } from '../../data'
import { say } from '../../assistant'

const props = defineProps<{ holder?: string; showHolder?: boolean; names?: Record<string, string> }>()
const query = ref('')
const showForgotten = ref(false)
const { data, error, loading, retry } = useRead('list_memories', () => ({
  holder: props.holder,
  query: query.value.trim() || undefined,
  include_forgotten: showForgotten.value,
}))
const KIND: Record<string, string> = {
  fact: 'Fact',
  preference: 'Preference',
  principle: 'Principle',
  lesson: 'Lesson',
  decision: 'Decision',
  intent: 'Intent',
}
const groups = computed(() => {
  const m = new Map<string, Memory[]>()
  for (const x of data.value ?? []) m.set(x.kind, [...(m.get(x.kind) ?? []), x])
  return [...m.entries()]
})
function forget(x: Memory) {
  say(`Forget memory #${x.id} “${x.title}” because `, props.holder ?? 'assistant')
}
</script>

<template>
  <div class="space-y-3">
    <div class="flex flex-wrap items-center gap-2">
      <label class="flex min-w-36 flex-1 items-center gap-2 rounded-lg border border-line bg-panel px-2.5 py-1.5 focus-within:border-violet">
        <Search :size="14" class="text-ink3" />
        <input v-model="query" aria-label="Search memories" class="min-w-0 flex-1 bg-transparent text-sm outline-none" placeholder="Search memories…" />
      </label>
      <label class="flex items-center gap-1 text-xs text-ink2"><input v-model="showForgotten" type="checkbox" /> Include forgotten</label>
    </div>
    <LoadState :loading="loading && !data" :error="error" label="memories" @retry="retry" />
    <p v-if="data && !data.length" class="text-sm text-ink3">
      {{ query.trim() ? 'No matching memories.' : 'No memories yet.' }}
    </p>
    <section v-for="[kind, items] in groups" :key="kind">
      <h3 class="mb-1 text-[11px] font-semibold uppercase tracking-wide text-ink3">{{ KIND[kind] ?? kind }} · {{ items.length }}</h3>
      <ul class="divide-y divide-line rounded-lg border border-line bg-panel">
        <li v-for="x in items" :key="x.id" class="group px-3 py-2" :class="x.ended ? 'opacity-55' : ''">
          <div class="flex items-start gap-2">
            <div class="min-w-0 flex-1">
              <div class="text-sm wrap-anywhere font-medium" :class="x.ended ? 'line-through' : ''">{{ x.title }}</div>
              <p v-if="x.body" class="mt-0.5 text-xs wrap-anywhere whitespace-pre-wrap text-ink2">{{ x.body }}</p>
              <div class="mt-1 flex flex-wrap gap-x-2 text-[11px] text-ink3">
                <span v-if="showHolder">{{ names?.[x.holder] ?? x.holder }}</span>
                <span>#{{ x.id }} · {{ x.updated }}</span>
                <span v-if="x.recalled">Recalled {{ x.recalled }} {{ x.recalled === 1 ? 'time' : 'times' }}</span>
                <span v-if="x.ended">Forgotten: {{ x.why_ended }}</span>
              </div>
            </div>
            <button
              v-if="!x.ended"
              type="button"
              class="shrink-0 rounded-full border border-line px-2 py-0.5 text-[11px] text-ink3 hover:border-violet hover:text-violet-text"
              @click="forget(x)"
            >
              Forget…
            </button>
          </div>
        </li>
      </ul>
    </section>
  </div>
</template>
