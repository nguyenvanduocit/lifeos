<script setup lang="ts">
import EntityId from '../components/ds/EntityId.vue'
// Any document read on its own (docs/design.md, "Reader"): notes, reviews, journal entries and whatever has no page of
// its own. A kind that has its own page (tasks, goals, people, crew: links.ts) is sent there.
import { computed, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { CalendarClock, ChevronRight, FileText, Sparkles, Tag } from '@lucide/vue'
import { useRead } from '../data'
import { docLink } from '../links'
import { longDay } from '../format'
import AskChip from '../components/AskChip.vue'
import LoadState from '../components/LoadState.vue'
import Markdown from '../components/Markdown.vue'
import PropertyList from '../components/ds/PropertyList.vue'

const props = defineProps<{ id: string }>()
const { data, error, loading, retry } = useRead('read_doc', () => ({ id: props.id }))
const route = useRoute()
const router = useRouter()

watch(data, (d) => {
  if (!d) return
  const own = docLink(d.summary.kind, d.summary.id)
  if (!own.startsWith('/doc/')) void router.replace(own)
}, { immediate: true })

const parent = computed(() => {
  if (typeof route.query.q === 'string') return { path: '/notes', name: 'Search', query: { q: route.query.q } }
  switch (data.value?.summary.kind) {
    case 'journal': return { path: '/journal', name: 'Journal' }
    case 'routine': return { path: '/automation', name: 'Automation' }
    case 'review': return { path: '/notes', name: 'Notes', query: { view: 'reviews' } }
    default: return { path: '/notes', name: 'Notes' }
  }
})
const KIND: Record<string, string> = { journal: 'Journal entry', note: 'Note', review: 'Review', routine: 'Routine' }
const day = computed(() => (/^\d{4}-\d{2}-\d{2}/.test(data.value?.summary.id ?? props.id) ? (data.value?.summary.id ?? props.id).slice(0, 10) : ''))
const words = computed(() => (data.value?.body.trim() ? data.value.body.trim().split(/\s+/).length : 0))
const rows = computed(() => {
  const s = data.value?.summary
  if (!s) return []
  return [
    { key: 'day', label: 'Date', icon: CalendarClock, value: day.value || s.due },
    { key: 'tags', label: 'Tags', icon: Tag, value: s.tags.length ? 'yes' : null },
    { key: 'path', label: 'File', icon: FileText, value: s.path },
  ]
})
</script>

<template>
  <nav aria-label="Breadcrumb" class="mb-6 flex items-center gap-1.5 text-[14px] text-ink2">
    <RouterLink :to="{ path: parent.path, query: parent.query }" class="hover:text-ink">{{ parent.name }}</RouterLink>
    <ChevronRight class="size-3.5 text-ink3" aria-hidden="true" />
    <span class="truncate text-ink" aria-current="page">{{ data?.summary.title ?? '…' }}</span>
  </nav>

  <LoadState :loading="loading && !data" :error="error" label="document" class="mb-4" @retry="retry" />

  <div v-if="data" class="@container max-w-[1080px]">
    <p class="mb-1 text-[14px] text-ink2">{{ KIND[data.summary.kind] ?? 'Document' }}<template v-if="words"> · {{ words }} words</template></p>
    <h1 class="text-[30px] leading-tight font-semibold tracking-[-0.02em] wrap-anywhere">{{ data.summary.title }}</h1>
    <EntityId :key="data.summary.path" :value="`${data.summary.kind}_${data.summary.id}`" />

    <div class="mt-8 grid items-start gap-8 @min-[52rem]:grid-cols-[minmax(0,1fr)_17rem]">
      <div class="order-2 rounded-2xl border border-line bg-panel2 px-4 py-4 @min-[52rem]:sticky @min-[52rem]:top-0">
        <p class="mb-2 text-[12.5px] font-medium text-ink3">Details</p>
        <PropertyList :rows="rows" stack>
          <template #day>{{ longDay(day || data.summary.due || '') }}</template>
          <template #tags>
            <span v-for="t in data.summary.tags" :key="t" class="rounded-md bg-panel px-1.5 py-0.5 text-[12.5px] text-ink2">#{{ t }}</span>
          </template>
          <template #path><span class="truncate font-mono text-[12px] text-ink3">{{ data.summary.path }}</span></template>
        </PropertyList>
        <div class="mt-4 flex flex-wrap gap-2 border-t border-line pt-4">
          <AskChip :icon="FileText" :words="`Update ${data.summary.kind}_${data.summary.id}: `" label="Edit" />
          <AskChip :icon="Sparkles" :words="`About “${data.summary.title}” (${data.summary.path}): `" label="Ask about it" />
        </div>
      </div>
      <article class="order-1 min-w-0">
        <Markdown v-if="data.body.trim()" class="max-w-[72ch] text-[15px] leading-relaxed wrap-anywhere" :source="data.body" />
        <p v-else class="rounded-xl border border-dashed border-line2 px-4 py-8 text-center text-[14px] text-ink3">This document is empty.</p>
      </article>
    </div>
  </div>
</template>
