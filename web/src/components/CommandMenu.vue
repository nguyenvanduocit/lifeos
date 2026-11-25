<script setup lang="ts">
// ⌘K (or /): one field for everything. First, "Ask the Assistant" (it drafts the words in the composer, as page
// actions do); then the pages; then what the vault's search finds. ↑↓ to move, ↵ to go, Esc to close.
import { computed, nextTick, ref, watch } from 'vue'
import { useRouter } from 'vue-router'
import { refDebounced } from '@vueuse/core'
import { DialogContent, DialogOverlay, DialogPortal, DialogRoot, DialogTitle } from 'reka-ui'
import { ArrowRight, CornerDownLeft, FileText, Goal, ListChecks, NotebookPen, Search, StickyNote, User } from '@lucide/vue'
import type { Component } from 'vue'
import { DOMAINS } from '../domains'
import { PAGES } from '../router'
import { read } from '../core/link'
import { say } from '../assistant'
import { docLink } from '../links'
import type { SearchHit } from '../core/generated/SearchHit'
import Kbd from './ds/Kbd.vue'

const open = defineModel<boolean>('open', { required: true })
const router = useRouter()
const q = ref('')
const input = ref<HTMLInputElement>()
const active = ref(0)
const hits = ref<SearchHit[]>([])
const searching = ref(false)
const typed = refDebounced(q, 120)

watch(open, async (o) => {
  if (!o) return
  q.value = ''
  hits.value = []
  active.value = 0
  await nextTick()
  input.value?.focus()
})

let seq = 0
watch(typed, async (text) => {
  const t = text.trim()
  const mine = ++seq
  if (t.length < 2) { hits.value = []; return }
  searching.value = true
  try {
    const out = await read('search', { query: t, limit: 8 })
    if (mine === seq) hits.value = out
  } catch {
    if (mine === seq) hits.value = []
  } finally {
    if (mine === seq) searching.value = false
  }
})

const KIND_ICON: Record<string, Component> = { task: ListChecks, goal: Goal, person: User, journal: NotebookPen, note: StickyNote }
interface Item { key: string; group: string; label: string; hint?: string; icon: Component; spark?: boolean; run: () => void }

const items = computed((): Item[] => {
  const t = q.value.trim()
  const lower = t.toLowerCase()
  const ask: Item[] = [{
    key: 'ask', group: 'Assistant', spark: true, icon: ArrowRight,
    label: t ? `Ask the Assistant: “${t}”` : 'Ask the Assistant…',
    hint: 'drafts it in the panel',
    run: () => say(t),
  }]
  const pages: Item[] = PAGES
    .filter((p) => !t || p.name.toLowerCase().includes(lower))
    .map((p) => ({ key: 'p' + p.path, group: 'Go to', label: p.name, icon: DOMAINS[p.domain].icon, hint: p.group, run: () => void router.push(p.path) }))
  const found: Item[] = hits.value.map((h) => ({
    key: 'h' + h.path, group: 'In your vault', label: h.title, icon: KIND_ICON[h.kind] ?? FileText,
    hint: h.kind, run: () => void router.push(docLink(h.kind, h.id)),
  }))
  return [...ask, ...(t ? [...found, ...pages] : pages)]
})
const groups = computed(() => {
  const out: { name: string; items: (Item & { index: number })[] }[] = []
  items.value.forEach((it, index) => {
    let g = out.find((x) => x.name === it.group)
    if (!g) out.push((g = { name: it.group, items: [] }))
    g.items.push({ ...it, index })
  })
  return out
})
watch(items, () => { if (active.value >= items.value.length) active.value = 0 })

function choose(i: number) {
  const it = items.value[i]
  if (!it) return
  open.value = false
  it.run()
}
function onKey(e: KeyboardEvent) {
  if (e.key === 'ArrowDown') { e.preventDefault(); active.value = (active.value + 1) % items.value.length }
  else if (e.key === 'ArrowUp') { e.preventDefault(); active.value = (active.value - 1 + items.value.length) % items.value.length }
  else if (e.key === 'Enter') { e.preventDefault(); choose(active.value) }
}
watch(active, async (i) => {
  await nextTick()
  document.getElementById(`cmd-${i}`)?.scrollIntoView({ block: 'nearest' })
})
</script>

<template>
  <DialogRoot v-model:open="open">
    <DialogPortal>
      <DialogOverlay class="fixed inset-0 z-50 bg-black/30 backdrop-blur-[2px] data-[state=open]:animate-in data-[state=open]:fade-in-0" />
      <DialogContent
        :aria-describedby="undefined"
        class="fixed top-[12vh] left-1/2 z-50 flex max-h-[70vh] w-[min(640px,calc(100vw-2rem))] -translate-x-1/2 flex-col overflow-hidden rounded-2xl border border-line2 bg-panel text-ink shadow-2xl shadow-black/30 data-[state=open]:animate-in data-[state=open]:fade-in-0 data-[state=open]:zoom-in-[0.98]"
      >
        <DialogTitle class="sr-only">Command menu</DialogTitle>
        <div class="flex h-14 shrink-0 items-center gap-3 border-b border-line px-4">
          <Search class="size-[18px] shrink-0 text-ink3" aria-hidden="true" />
          <input
            ref="input"
            v-model="q"
            role="combobox"
            aria-expanded="true"
            aria-controls="cmd-list"
            :aria-activedescendant="`cmd-${active}`"
            aria-label="Ask LifeOS or jump to"
            placeholder="Ask LifeOS or jump to…"
            class="min-w-0 flex-1 bg-transparent text-[16px] outline-none placeholder:text-ink3"
            @keydown="onKey"
          />
          <span v-if="searching" class="size-3.5 shrink-0 animate-spin rounded-full border-2 border-line2 border-t-ink2" aria-label="Searching" />
          <Kbd>esc</Kbd>
        </div>
        <div id="cmd-list" role="listbox" class="min-h-0 flex-1 overflow-y-auto p-2">
          <div v-for="g in groups" :key="g.name" class="mb-1 last:mb-0">
            <p class="px-2.5 pt-2 pb-1.5 text-[12px] font-medium text-ink3">{{ g.name }}</p>
            <div
              v-for="it in g.items"
              :id="`cmd-${it.index}`"
              :key="it.key"
              role="option"
              :aria-selected="active === it.index"
              class="flex h-10 cursor-pointer items-center gap-3 rounded-[10px] px-2.5 text-[14px] aria-selected:bg-panel2"
              @mousemove="active = it.index"
              @click="choose(it.index)"
            >
              <svg v-if="it.spark" viewBox="0 0 24 24" fill="currentColor" class="size-4 shrink-0 text-ink" aria-hidden="true">
                <path d="M12 1.5c.4 5.6 4.9 10.1 10.5 10.5-5.6.4-10.1 4.9-10.5 10.5C11.6 16.9 7.1 12.4 1.5 12 7.1 11.6 11.6 7.1 12 1.5Z" />
              </svg>
              <component :is="it.icon" v-else class="size-4 shrink-0 text-ink2" aria-hidden="true" />
              <span class="min-w-0 flex-1 truncate" :class="it.spark ? 'font-medium' : ''">{{ it.label }}</span>
              <span v-if="it.hint" class="shrink-0 text-[12.5px] text-ink3 first-letter:uppercase">{{ it.hint }}</span>
              <CornerDownLeft v-if="active === it.index" class="size-3.5 shrink-0 text-ink3" aria-hidden="true" />
            </div>
          </div>
        </div>
        <div class="flex h-10 shrink-0 items-center gap-4 border-t border-line bg-panel2 px-4 text-[12px] text-ink3">
          <span class="flex items-center gap-1.5"><Kbd>↑</Kbd><Kbd>↓</Kbd> move</span>
          <span class="flex items-center gap-1.5"><Kbd>↵</Kbd> open</span>
          <span class="ml-auto">Search covers tasks, goals, people, journal and notes</span>
        </div>
      </DialogContent>
    </DialogPortal>
  </DialogRoot>
</template>
