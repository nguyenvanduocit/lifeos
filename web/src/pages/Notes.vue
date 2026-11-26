<script setup lang="ts">
// Notes, briefs and reviews as note apps lay them out (docs/design.md, "List + reader"): a list pane and a reader pane
// that fill the canvas, each scrolling on its own, and search across the whole vault at the top of the list. The view,
// the search words and the open note live in the address (?view=, ?q=, ?peek=); with no ?peek= the reader shows the
// first row. Below a 48rem page the list is the page and a row opens the note's own page (/doc/:id).
import { computed, ref, watch } from 'vue'
import { createReusableTemplate, useElementSize, watchDebounced } from '@vueuse/core'
import { useRoute } from 'vue-router'
import { ClipboardCheck, ExternalLink, FileText, Plus, Search, Sparkles, StickyNote, X } from '@lucide/vue'
import { useRead } from '../data'
import { useQueryRef } from '../nav'
import { docLink } from '../links'
import { DOMAINS } from '../domains'
import { longDay, wordCount } from '../format'
import AskChip from '../components/AskChip.vue'
import LoadState from '../components/LoadState.vue'
import Markdown from '../components/Markdown.vue'
import PageHeader from '../components/ds/PageHeader.vue'
import Segmented from '../components/ds/Segmented.vue'
import IconTile from '../components/ds/IconTile.vue'
import type { Kind } from '../core/generated/Kind'

const route = useRoute()
const VIEWS = ['notes', 'reviews'] as const
const view = useQueryRef('view', 'notes', VIEWS)
const q = useQueryRef('q', '')
const typed = ref(q.value)
watch(q, (v) => { if (v !== typed.value) typed.value = v })
watchDebounced(typed, (v) => { q.value = v }, { debounce: 200 })
const searching = computed(() => !!q.value.trim())
const peek = computed(() => (typeof route.query.peek === 'string' ? route.query.peek : ''))
// The panes themselves are laid out by container classes (the Assistant column changes the page's width, not the
// window's); this mirror of the 48rem step only says where a row leads and whether there is a reader to fill.
const root = ref<HTMLElement>()
const { width } = useElementSize(root)
const twoPane = computed(() => width.value >= 768)

const { data: notes, error: notesError, loading: notesLoading, retry: retryNotes } = useRead('recent_notes', () => ({ limit: 100 }))
const { data: reviews, error: reviewsError, loading: reviewsLoading, retry: retryReviews } = useRead('reviews', () => ({ limit: 50 }))
const { data: hits, error: hitsError, loading: hitsLoading, retry: retryHits } = useRead('search', () => ({ query: q.value.trim(), limit: 30 }))

const ICON: Partial<Record<Kind, typeof FileText>> = { note: StickyNote, review: ClipboardCheck }
const iconOf = (k: Kind) => ICON[k] ?? DOMAINS[({ task: 'tasks', goal: 'goals', person: 'people', journal: 'journal', crew: 'crew', routine: 'automation' } as Record<string, keyof typeof DOMAINS>)[k] ?? 'notes'].icon
const KIND: Record<Kind, string> = { task: 'Task', goal: 'Goal', person: 'Person', journal: 'Journal', note: 'Note', review: 'Review', crew: 'Crew', routine: 'Routine' }
const REVIEW: Record<string, string> = { brief: 'Brief', weekly: 'Weekly', monthly: 'Monthly' }
const dateOf = (id: string) => (/^\d{4}-\d{2}-\d{2}/.test(id) ? id.slice(0, 10) : '')
const tagLine = (tags: string[]) => tags.map((t) => '#' + t).join('  ')

// A row is a title and one quiet line: a note's tags, a review's day and kind, a search hit's kind and the match.
interface Row { id: string; kind: Kind; title: string; sub: string }
const rows = computed<Row[]>(() => {
  if (searching.value) return (hits.value ?? []).map((h) => ({ id: h.id, kind: h.kind, title: h.title, sub: [KIND[h.kind], h.snippet.replace(/[«»]/g, '').replace(/(^|\s)#+\s/g, ' ').trim()].filter(Boolean).join(' · ') }))
  if (view.value === 'reviews') return (reviews.value ?? []).map((r) => ({ id: r.id, kind: r.kind, title: r.title, sub: [dateOf(r.id) && longDay(dateOf(r.id)), REVIEW[r.status ?? ''] ?? 'Review'].filter(Boolean).join(' · ') }))
  return (notes.value ?? []).map((n) => ({ id: n.id, kind: n.kind, title: n.title, sub: tagLine(n.tags) }))
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
const nothing = computed(() => (searching.value ? `Nothing matches “${q.value.trim()}”.` : view.value === 'reviews' ? 'No briefs or reviews yet.' : 'No notes yet.'))

// Notes and reviews are read here; other kinds (tasks, people…) have a page of their own.
const handle = (r: { id: string; kind: Kind }) => `${r.kind}_${r.id}`
const readable = (r: { id: string; kind: Kind }) => docLink(r.kind, r.id).startsWith('/doc/')
// A page opened from a search keeps the words, so its breadcrumb leads back to the results (Doc.vue).
const pageOf = (id: string) => ({ path: `/doc/${encodeURIComponent(id)}`, query: searching.value ? { q: q.value.trim() } : undefined })
const inReader = (r: Row) => twoPane.value && readable(r)
const rowLink = (r: Row) => {
  if (!readable(r)) return docLink(r.kind, r.id)
  return twoPane.value ? { query: { ...route.query, peek: handle(r) } } : pageOf(handle(r))
}
// The reader always shows a note: the one the address names, else the first row that can be read here. That first
// row is then held: a list that refreshes under the reader (a note saved meanwhile comes first) does not swap what is
// being read. It moves on when the view or the search changes, or when the held note leaves the list. Only a pick is
// written to the address.
const held = ref('')
watch([rows, view, () => q.value.trim()], ([list, v, sought], [, v0, sought0]) => {
  if (v === v0 && sought === sought0 && list.some((r) => handle(r) === held.value)) return
  const first = list.find(readable)
  held.value = first ? handle(first) : ''
}, { immediate: true })
const shown = computed(() => (twoPane.value ? peek.value || held.value : ''))
const isShown = (r: Row) => !!shown.value && (shown.value === handle(r) || shown.value === r.id)
const { data: doc, error: docError, loading: docLoading, retry: retryDoc } = useRead('read_doc', () => ({ id: shown.value || '__none__' }))
const words = computed(() => wordCount(doc.value?.body ?? ''))
const about = computed(() => {
  const s = doc.value?.summary
  if (!s) return 'Note'
  return [KIND[s.kind], s.kind === 'review' && dateOf(s.id) && longDay(dateOf(s.id))].filter(Boolean).join(' · ')
})
// Another note starts at its top; the same note read again (the data changed) keeps its place.
const scroller = ref<HTMLElement>()
watch(shown, () => { if (scroller.value) scroller.value.scrollTop = 0 })
// The empty state, once: in the reader when there are two panes, in the list when the list is the page.
const [DefineEmpty, Empty] = createReusableTemplate<{ line: string }>()
</script>

<template>
  <PageHeader title="Notes">
    <AskChip size="sm" :icon="Sparkles" words="Write my weekly review." label="Weekly review" />
    <AskChip size="sm" primary :icon="Plus" words="Save a note: " label="New note" />
  </PageHeader>

  <DefineEmpty v-slot="{ line }">
    <div class="flex flex-1 flex-col items-center justify-center gap-3 px-5 py-14 text-center">
      <span class="grid size-11 place-items-center rounded-xl bg-panel2"><StickyNote class="size-5 text-ink2" /></span>
      <p class="text-[15px] font-medium">{{ line }}</p>
      <AskChip v-if="!searching" :icon="Plus" :words="view === 'reviews' ? 'Write my weekly review.' : 'Save a note: '" :label="view === 'reviews' ? 'Write a review' : 'New note'" />
    </div>
  </DefineEmpty>

  <!-- The panes take what height the page has left under any page widgets, and never less than 324px: the list's two
       48px bars and four 57px rows (four is a choice, no outside source). When widgets leave less, the page scrolls.
       The list is 340px, 300px below a 64rem page, and the reader comes at 48rem: widths chosen for this page (no
       outside source); at 340px a 44-character title fits on one line. -->
  <div ref="root" class="grid min-h-[324px] flex-1 grid-cols-1 grid-rows-[minmax(0,1fr)] @min-[48rem]:grid-cols-[300px_minmax(0,1fr)] @min-[64rem]:grid-cols-[340px_minmax(0,1fr)]">
    <section aria-label="Notes list" class="flex min-h-0 min-w-0 flex-col @min-[48rem]:border-r @min-[48rem]:border-line">
      <div class="flex h-12 shrink-0 items-center border-b border-line px-4">
        <label class="flex h-8 w-full items-center gap-2 rounded-[10px] border border-line2 bg-panel px-2.5 focus-within:outline-2 focus-within:outline-ring">
          <Search class="size-4 shrink-0 text-ink3" aria-hidden="true" />
          <input v-model="typed" aria-label="Search the vault" placeholder="Search everything…" class="min-w-0 flex-1 bg-transparent text-[13px] outline-none placeholder:text-ink3" />
          <button v-if="typed" type="button" class="grid size-6 place-items-center rounded text-ink3 hover:text-ink" aria-label="Clear search" @click="typed = ''; q = ''"><X class="size-3.5" /></button>
        </label>
      </div>
      <div class="flex h-12 shrink-0 items-center border-b border-line px-4">
        <Segmented v-if="!searching" v-model="view" :options="options" label="View" />
        <p v-else class="min-w-0 truncate text-[13px] font-medium" aria-live="polite">Results for “{{ q.trim() }}” <span class="text-ink3 tabular-nums">{{ hits?.length ?? '' }}</span></p>
      </div>

      <div class="min-h-0 flex-1 overflow-y-auto">
        <LoadState :loading="state.loading" :error="state.error" :label="searching ? 'results' : view" class="mx-4 my-3" @retry="state.retry()" />
        <template v-if="!state.loading && !state.error">
          <ul v-if="rows.length" :aria-label="searching ? 'Search results' : view === 'reviews' ? 'Briefs and reviews' : 'Notes'">
            <li v-for="r in rows" :key="r.kind + r.id" class="border-b border-line">
              <RouterLink :to="rowLink(r)" :replace="inReader(r)" :title="r.title" :aria-current="isShown(r) ? 'true' : undefined"
                class="relative flex min-h-14 items-center gap-3 px-4 py-2.5 transition-colors hover:bg-panel2/70 focus-visible:outline-2 focus-visible:-outline-offset-2 focus-visible:outline-ring"
                :class="isShown(r) ? 'bg-panel2' : ''">
                <span v-if="isShown(r)" class="absolute inset-y-0 left-0 w-0.5 bg-ink" aria-hidden="true" />
                <IconTile v-if="searching" :icon="iconOf(r.kind)" :tone="r.kind === 'review' ? 'signal' : 'notes'" size="sm" class="self-start" />
                <span class="min-w-0 flex-1">
                  <span class="block truncate text-[14.5px] font-medium">{{ r.title }}</span>
                  <span v-if="r.sub" class="text-[13px] text-ink3" :class="searching ? 'line-clamp-2' : 'block truncate'">{{ r.sub }}</span>
                </span>
              </RouterLink>
            </li>
          </ul>
          <!-- Beside a reader the list only says it is empty; alone, it carries the way to a first note. -->
          <p v-else-if="twoPane" class="px-4 py-4 text-[14px] text-ink3">{{ nothing }}</p>
          <Empty v-else :line="nothing" />
        </template>
      </div>
    </section>

    <section v-if="twoPane" aria-label="Note" class="flex min-h-0 min-w-0 flex-col">
      <template v-if="shown">
        <div class="flex h-12 shrink-0 items-center gap-1.5 border-b border-line px-4">
          <span class="min-w-0 truncate text-[13px] text-ink3">{{ about }}</span>
          <div class="ml-auto flex shrink-0 items-center gap-1.5">
            <template v-if="doc">
              <AskChip size="sm" :icon="FileText" :words="`Update ${doc.summary.kind}_${doc.summary.id}: `" label="Edit" />
              <AskChip size="sm" :icon="Sparkles" :words="`Summarise “${doc.summary.title}” (${doc.summary.path}) and tell me what to do with it.`" label="Summarise" />
            </template>
            <RouterLink :to="pageOf(shown)" class="inline-flex h-8 items-center gap-1.5 rounded-lg px-2.5 text-[13px] text-ink2 hover:bg-panel2 hover:text-ink focus-visible:outline-2 focus-visible:outline-ring">
              <ExternalLink class="size-3.5" aria-hidden="true" /> Open page
            </RouterLink>
          </div>
        </div>
        <div ref="scroller" class="min-h-0 flex-1 overflow-y-auto px-8 pt-8 pb-16 @min-[64rem]:px-10">
          <div class="reader mx-auto w-full">
            <LoadState :loading="docLoading && !doc" :error="docError" label="note" @retry="retryDoc" />
            <article v-if="doc" aria-labelledby="note-title">
              <h2 id="note-title" class="text-[28px] leading-tight font-semibold tracking-[-0.02em] wrap-anywhere">{{ doc.summary.title }}</h2>
              <div v-if="words || doc.summary.tags.length" class="mt-2 flex flex-wrap items-center gap-x-2.5 gap-y-1.5 text-[13px] text-ink2">
                <span v-if="words" class="tabular-nums">{{ words }} words</span>
                <span v-for="t in doc.summary.tags" :key="t" class="rounded-md bg-panel2 px-1.5 py-0.5 text-[12.5px]">#{{ t }}</span>
              </div>
              <Markdown v-if="doc.body.trim()" :source="doc.body" class="mt-6 wrap-anywhere" />
              <p v-else class="mt-6 text-[14px] text-ink3">This note is empty.</p>
            </article>
          </div>
        </div>
      </template>
      <Empty v-else-if="!state.loading && !state.error" :line="rows.length ? 'These results open on their own pages.' : searching ? 'Nothing matches.' : nothing" />
    </section>
  </div>
</template>
