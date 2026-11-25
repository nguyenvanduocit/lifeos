<script setup lang="ts">
// What an agent remembers, read-only. Forgetting is asked of the agent.
import { computed, ref } from 'vue'
import { Search } from '@lucide/vue'
import type { Memory } from '../../core/generated/Memory'
import { useRead } from '../../data'
import { say } from '../../assistant'

const props = defineProps<{ holder?: string; showHolder?: boolean; names?: Record<string, string> }>()
const query = ref('')
const showForgotten = ref(false)
const { data } = useRead('list_memories', () => ({
  holder: props.holder,
  query: query.value.trim() || undefined,
  include_forgotten: showForgotten.value,
}))
const KIND: Record<string, string> = {
  fact: 'sự thật',
  preference: 'sở thích',
  principle: 'nguyên tắc',
  lesson: 'bài học',
  decision: 'quyết định',
  intent: 'dự định',
}
const groups = computed(() => {
  const m = new Map<string, Memory[]>()
  for (const x of data.value ?? []) m.set(x.kind, [...(m.get(x.kind) ?? []), x])
  return [...m.entries()]
})
function forget(x: Memory) {
  say(`Quên memory #${x.id} “${x.title}” vì `, props.holder ?? 'assistant')
}
</script>

<template>
  <div class="space-y-3">
    <div class="flex items-center gap-2">
      <label class="flex flex-1 items-center gap-2 rounded-lg border border-line bg-panel px-2.5 py-1.5 focus-within:border-violet">
        <Search :size="14" class="text-ink3" />
        <input v-model="query" class="flex-1 bg-transparent text-sm outline-none" placeholder="Tìm trong memory…" />
      </label>
      <label class="flex items-center gap-1 text-xs text-ink2"><input v-model="showForgotten" type="checkbox" /> đã quên</label>
    </div>
    <p v-if="data && !data.length" class="text-sm text-ink3">
      Chưa nhớ điều gì{{ query ? ' khớp' : '' }}. Agent tự ghi nhớ khi bạn kể về mình: sở thích, nguyên tắc, quyết định, dự định.
    </p>
    <section v-for="[kind, items] in groups" :key="kind">
      <h3 class="mb-1 text-[11px] font-semibold uppercase tracking-wide text-ink3">{{ KIND[kind] ?? kind }} · {{ items.length }}</h3>
      <ul class="divide-y divide-line rounded-lg border border-line bg-panel">
        <li v-for="x in items" :key="x.id" class="group px-3 py-2" :class="x.ended ? 'opacity-55' : ''">
          <div class="flex items-start gap-2">
            <div class="min-w-0 flex-1">
              <div class="text-sm font-medium" :class="x.ended ? 'line-through' : ''">{{ x.title }}</div>
              <p v-if="x.body" class="mt-0.5 text-xs text-ink2 whitespace-pre-wrap">{{ x.body }}</p>
              <div class="mt-1 flex flex-wrap gap-x-2 text-[11px] text-ink3">
                <span v-if="showHolder">{{ names?.[x.holder] ?? x.holder }}</span>
                <span>#{{ x.id }} · {{ x.updated }}</span>
                <span v-if="x.recalled">nhớ lại {{ x.recalled }} lần</span>
                <span v-if="x.ended">đã quên: {{ x.why_ended }}</span>
              </div>
            </div>
            <button
              v-if="!x.ended"
              type="button"
              class="shrink-0 rounded-full border border-line px-2 py-0.5 text-[11px] text-ink3 opacity-0 group-hover:opacity-100 hover:border-violet hover:text-violet-text"
              @click="forget(x)"
            >
              Quên…
            </button>
          </div>
        </li>
      </ul>
    </section>
  </div>
</template>
