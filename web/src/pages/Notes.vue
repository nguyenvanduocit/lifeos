<script setup lang="ts">
import EntityId from '../components/ds/EntityId.vue'
// Notes, briefs and reviews as a list with a peek (docs/design.md, "List + peek"), and search across the whole vault.
// The view, the search words and the open note live in the address (?view=, ?q=, ?peek=); on a wide screen a row opens
// beside the list, on a narrow one on its own page.
import { computed, ref, watch } from 'vue'
import { useMediaQuery, watchDebounced } from '@vueuse/core'
import { useRoute, useRouter } from 'vue-router'
import { ClipboardCheck, ExternalLink, FileText, Hash, Plus, Search, Sparkles, StickyNote, X } from '@lucide/vue'
import { useRead } from '../data'
import { useQueryRef } from '../nav'
import { docLink } from '../links'
import { DOMAINS } from '../domains'
import { longDay } from '../format'
import AskChip from '../components/AskChip.vue'
import LoadState from '../components/LoadState.vue'
import Markdown from '../components/Markdown.vue'
import PageHeader from '../components/ds/PageHeader.vue'
import Segmented from '../components/ds/Segmented.vue'
import TableHead from '../components/ds/TableHead.vue'
import IconTile from '../components/ds/IconTile.vue'
import type { Kind } from '../core/generated/Kind'

const route = useRoute()
const router = useRouter()
const VIEWS = ['notes', 'reviews'] as const
const view = useQueryRef('view', 'notes', VIEWS)
const q = useQueryRef('q', '')
const typed = ref(q.value)
watch(q, (v) => { if (v !== typed.value) typed.value = v })
watchDebounced(typed, (v) => { q.value = v }, { debounce: 200 })
const searching = computed(() => !!q.value.trim())
const peek = computed(() => (typeof route.query.peek === 'string' ? route.query.peek : ''))
const wide = useMediaQuery('(min-width: 64rem)')

const { data: notes, error: notesError, loading: notesLoading, retry: retryNotes } = useRead('recent_notes', () => ({ limit: 100 }))
const { data: reviews, error: reviewsError, loading: reviewsLoading, retry: retryReviews } = useRead('reviews', () => ({ limit: 50 }))
const { data: hits, error: hitsError, loading: hitsLoading, retry: retryHits } = useRead('search', () => ({ query: q.value.trim(), limit: 30 }))

const ICON: Partial<Record<Kind, typeof FileText>> = { note: StickyNote, review: ClipboardCheck }
const iconOf = (k: Kind) => ICON[k] ?? DOMAINS[({ task: 'tasks', goal: 'goals', person: 'people', journal: 'journal', crew: 'crew', routine: 'automation' } as Record<string, keyof typeof DOMAINS>)[k] ?? 'notes'].icon
const KIND: Record<Kind, string> = { task: 'Task', goal: 'Goal', person: 'Person', journal: 'Journal', note: 'Note', review: 'Review', crew: 'Crew', routine: 'Routine' }
const dateOf = (id: string) => (/^\d{4}-\d{2}-\d{2}/.test(id) ? id.slice(0, 10) : '')

const rows = computed(() => {
  if (searching.value) return (hits.value ?? []).map((h) => ({ id: h.id, kind: h.kind, title: h.title, sub: h.snippet.replace(/[«»]/g, '').replace(/(^|\s)#+\s/g, ' ').trim(), when: KIND[h.kind] }))
  if (view.value === 'reviews') return (reviews.value ?? []).map((r) => ({ id: r.id, kind: r.kind, title: r.title, sub: r.tags.map((t) => '#' + t).join('  '), when: dateOf(r.id) ? longDay(dateOf(r.id)) : '' }))
  return (notes.value ?? []).map((n) => ({ id: n.id, kind: n.kind, title: n.title, sub: n.tags.map((t) => '#' + t).join('  '), when: dateOf(n.id) ? longDay(dateOf(n.id)) : '' }))
})
const state = computed(() => searching.value
  ? { loading: hitsLoading.value && !hits.value, error: hitsError.value, retry: retryHits }
  : view.value === 'reviews'
    ? { loading: reviewsLoading.value && !reviews.value, error: reviewsError.value, retry: retryReviews }
    : { loading: notesLoading.value && !notes.value, error: notesError.value, retry: retryNotes })
const options = computed(() => [
  { value: 'notes' as const, label: 'Notes', count: notes.value?.length },
  { value: 'reviews' as const, label: 'Briefs & reviews', count: reviews.value?.length },
])

const second = computed(() => rows.value.some((r) => r.when))
const GRID = computed(() => (second.value ? 'minmax(0,3fr) minmax(0,1fr)' : 'minmax(0,1fr)'))
const cols = computed(() => [
  { label: searching.value ? 'Result' : view.value === 'reviews' ? 'Review' : 'Note', icon: FileText },
  ...(second.value ? [{ label: searching.value ? 'Kind' : 'Date', icon: Hash }] : []),
])
const isPeeked = (r: { id: string; kind: Kind }) =>
  peek.value === `${r.kind}_${r.id}` || peek.value === r.id
// Notes and reviews open in the peek; other kinds (tasks, people…) go to their own page.
const rowLink = (r: { id: string; kind: Kind }) => {
  const own = docLink(r.kind, r.id)
  if (!own.startsWith('/doc/') || !wide.value) return own
  return { query: { ...route.query, peek: `${r.kind}_${r.id}` } }
}
const { data: peeked, error: peekError, loading: peekLoading, retry: retryPeek } = useRead('read_doc', () => ({ id: peek.value || '__none__' }))
function closePeek() {
  void router.replace({ query: { ...route.query, peek: undefined } })
}
</script>

<template>
  <PageHeader title="Notes" :eyebrow="notes && reviews ? `${notes.length} notes · ${reviews.length} briefs and reviews` : undefined">
    <AskChip size="sm" :icon="Sparkles" words="Write my weekly review." label="Weekly review" />
    <AskChip size="sm" primary :icon="Plus" words="Save a note: " label="New note" />
  </PageHeader>

  <div class="grid items-start gap-5" :class="peek && wide ? 'grid-cols-[minmax(0,1fr)_460px]' : ''">
    <section class="@container min-w-0 overflow-hidden rounded-2xl border border-line bg-panel">
      <div class="flex flex-wrap items-center gap-3 px-5 py-3.5">
        <div v-if="!searching" class="max-w-full overflow-x-auto [scrollbar-width:none]"><Segmented v-model="view" :options="options" label="View" /></div>
        <span v-if="searching" class="text-[14px] font-medium">Results for “{{ q.trim() }}” <span class="text-ink3 tabular-nums">{{ hits?.length ?? '' }}</span></span>
        <label class="ml-auto flex h-8 w-full items-center gap-2 rounded-[10px] border border-line2 bg-panel px-2.5 focus-within:outline-2 focus-within:outline-ring @min-[36rem]:w-64">
          <Search class="size-4 shrink-0 text-ink3" aria-hidden="true" />
          <input v-model="typed" aria-label="Search the vault" placeholder="Search everything…" class="min-w-0 flex-1 bg-transparent text-[13px] outline-none placeholder:text-ink3" />
          <button v-if="typed" type="button" class="grid size-6 place-items-center rounded text-ink3 hover:text-ink" aria-label="Clear search" @click="typed = ''; q = ''"><X class="size-3.5" /></button>
        </label>
      </div>

      <LoadState :loading="state.loading" :error="state.error" :label="searching ? 'results' : view" class="px-5 pb-4" @retry="state.retry()" />
      <div v-if="!state.loading && !state.error" role="table" :aria-label="searching ? 'Search results' : view">
        <TableHead :grid="GRID" :grid-sm="second ? undefined : 'minmax(0,1fr)'" :cols="cols" />
        <div v-for="r in rows" :key="r.kind + r.id" role="row"
          class="relative grid min-h-[60px] items-center gap-x-4 border-b border-line px-5 py-2.5 transition-colors last:border-b-0 hover:bg-panel2/70"
          :class="isPeeked(r) ? 'bg-panel2' : ''" :style="{ '--cols': GRID, '--cols-sm': second ? 'minmax(0,1fr) auto' : 'minmax(0,1fr)' }" data-cols>
          <span v-if="isPeeked(r)" class="absolute inset-y-0 left-0 w-0.5 bg-ink" aria-hidden="true" />
          <div role="cell" class="flex min-w-0 items-center gap-3.5">
            <IconTile :icon="iconOf(r.kind)" :tone="r.kind === 'review' ? 'signal' : 'notes'" size="sm" />
            <RouterLink :to="rowLink(r)" class="min-w-0 after:absolute after:inset-0 focus-visible:outline-none" :aria-current="isPeeked(r) ? 'true' : undefined">
              <span class="block truncate text-[14.5px] font-medium">{{ r.title }}</span>
              <span v-if="r.sub" class="block truncate text-[13px] text-ink3">{{ r.sub }}</span>
            </RouterLink>
          </div>
          <div v-if="second" role="cell" class="text-[13.5px] whitespace-nowrap text-ink2">{{ r.when }}</div>
        </div>
        <div v-if="!rows.length" class="flex flex-col items-center gap-3 px-5 py-14 text-center">
          <span class="grid size-11 place-items-center rounded-xl bg-panel2"><StickyNote class="size-5 text-ink2" /></span>
          <p class="text-[15px] font-medium">{{ searching ? 'Nothing matches.' : view === 'reviews' ? 'No briefs or reviews yet.' : 'No notes yet.' }}</p>
          <AskChip v-if="!searching" :icon="Plus" :words="view === 'reviews' ? 'Write my weekly review.' : 'Save a note: '" :label="view === 'reviews' ? 'Write a review' : 'New note'" />
        </div>
      </div>
    </section>

    <aside v-if="peek && wide" class="sticky top-0 flex h-[calc(100dvh-8.5rem)] flex-col overflow-hidden rounded-2xl border border-line bg-panel shadow-xl shadow-black/5" aria-label="Note">
      <div class="flex h-14 shrink-0 items-center gap-1 border-b border-line px-4">
        <span class="text-[13px] text-ink3">{{ peeked ? KIND[peeked.summary.kind] : 'Note' }}</span>
        <RouterLink :to="`/doc/${encodeURIComponent(peek)}`" class="ml-auto inline-flex h-8 items-center gap-1.5 rounded-lg px-2.5 text-[13px] text-ink2 hover:bg-panel2 hover:text-ink">
          <ExternalLink class="size-3.5" aria-hidden="true" /> Open page
        </RouterLink>
        <button type="button" class="grid size-8 place-items-center rounded-lg text-ink2 hover:bg-panel2 hover:text-ink" aria-label="Close" @click="closePeek"><X class="size-4" /></button>
      </div>
      <div class="min-h-0 flex-1 overflow-y-auto px-6 py-5">
        <LoadState :loading="peekLoading && !peeked" :error="peekError" label="note" @retry="retryPeek" />
        <template v-if="peeked">
          <h2 class="text-[22px] leading-snug font-semibold tracking-[-0.02em] wrap-anywhere">{{ peeked.summary.title }}</h2>
          <EntityId :key="peeked.summary.path" :value="`${peeked.summary.kind}_${peeked.summary.id}`" />
          <div v-if="peeked.summary.tags.length" class="mt-2 flex flex-wrap gap-1.5">
            <span v-for="t in peeked.summary.tags" :key="t" class="rounded-md bg-panel2 px-1.5 py-0.5 text-[12.5px] text-ink2">#{{ t }}</span>
          </div>
          <Markdown v-if="peeked.body.trim()" :source="peeked.body" class="mt-5 text-[14.5px] leading-relaxed wrap-anywhere" />
          <p v-else class="mt-5 text-[14px] text-ink3">This note is empty.</p>
          <div class="mt-6 flex flex-wrap gap-2 border-t border-line pt-5">
            <AskChip :icon="FileText" :words="`Update ${peeked.summary.kind}_${peeked.summary.id}: `" label="Edit" />
            <AskChip :icon="Sparkles" :words="`Summarise “${peeked.summary.title}” (${peeked.summary.path}) and tell me what to do with it.`" label="Summarise" />
          </div>
        </template>
      </div>
    </aside>
  </div>
</template>
