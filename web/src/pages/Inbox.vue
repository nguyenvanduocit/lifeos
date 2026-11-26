<script setup lang="ts">
// The Inbox as a triage list (docs/design.md, "List"): what was captured, newest first, grouped by day. Each row reads
// the item for its parts (where it came from, a title, the rest, a suggested step) and offers the ways out of the
// Inbox as Assistant drafts. A source filter lives in the address (?source=).
import { computed, ref } from 'vue'
import { useIntervalFn } from '@vueuse/core'
import { ArrowRight, Check, CheckCheck, Hourglass, Inbox, Lightbulb, ListPlus, Mail, Plus, StickyNote, Wand2 } from '@lucide/vue'
import { useRead } from '../data'
import { useQueryRef } from '../nav'
import { isoDay, longDay, relativeDay, todayIso } from '../format'
import AskChip from '../components/AskChip.vue'
import LoadState from '../components/LoadState.vue'
import PageHeader from '../components/ds/PageHeader.vue'
import StatStrip from '../components/ds/StatStrip.vue'
import Segmented from '../components/ds/Segmented.vue'
import IconTile from '../components/ds/IconTile.vue'
import { SOURCES, ago, createdAt, parseInbox, type Source } from '../components/capture/inbox'

const { data, error, loading, retry } = useRead('list_inbox')
const now = ref(new Date())
useIntervalFn(() => (now.value = new Date()), 60_000)

const items = computed(() => (data.value ?? []).map((i) => {
  const at = createdAt(i.created)
  return { ...i, at, day: isNaN(at.getTime()) ? i.created.slice(0, 10) : isoDay(at), parsed: parseInbox(i.text) }
}))

const FILTERS = ['all', 'mail', 'idea', 'read', 'link', 'note'] as const
const source = useQueryRef('source', 'all', FILTERS)
const bySource = computed(() => {
  const n: Partial<Record<Source, number>> = {}
  for (const i of items.value) n[i.parsed.source] = (n[i.parsed.source] ?? 0) + 1
  return n
})
const filterOptions = computed(() => [
  { value: 'all' as const, label: 'All', count: items.value.length },
  ...(Object.keys(SOURCES) as Source[]).filter((s) => bySource.value[s]).map((s) => ({ value: s, label: SOURCES[s].label, count: bySource.value[s] })),
])
const shown = computed(() => items.value.filter((i) => source.value === 'all' || i.parsed.source === source.value))

const groups = computed(() => {
  const out: { day: string; label: string; items: typeof items.value }[] = []
  for (const it of shown.value) {
    const g = out.find((x) => x.day === it.day)
    if (g) g.items.push(it)
    else out.push({ day: it.day, label: it.day === todayIso() ? 'Today' : `${relativeDay(it.day)} · ${longDay(it.day)}`, items: [it] })
  }
  return out
})

const oldest = computed(() => items.value.reduce<Date | null>((o, i) => (!o || i.at < o ? i.at : o), null))
const strip = computed(() => {
  if (!items.value.length) return []
  return [
    { key: 'waiting', icon: Inbox, value: String(items.value.length), label: items.value.length === 1 ? 'item waiting' : 'items waiting', urgent: items.value.length > 0 },
    ...(oldest.value ? [{ key: 'oldest', icon: Hourglass, value: ago(oldest.value, now.value).replace(' ago', '').replace(/^(\d+) (\w)\w*\.?$/, '$1$2'), label: 'oldest waiting' }] : []),
    { key: 'mail', icon: Mail, value: String(bySource.value.mail ?? 0), label: 'from email' },
    { key: 'idea', icon: Lightbulb, value: String(bySource.value.idea ?? 0), label: 'ideas' },
  ]
})
const ref_ = (i: { id: number; text: string }) => `inbox_${i.id}`
</script>

<template>
  <PageHeader title="Inbox" :eyebrow="data ? (data.length ? `${data.length} to triage` : 'Nothing to triage') : undefined">
    <AskChip v-if="data?.length" size="sm" :icon="Wand2" words="Suggest how to triage my Inbox for my approval." label="Triage all" />
    <AskChip size="sm" primary :icon="Plus" words="Capture in Inbox: " label="Capture" />
    <template #strip><StatStrip v-if="strip.length" :items="strip" /></template>
  </PageHeader>

  <LoadState :loading="loading && !data" :error="error" label="inbox" class="mb-4" @retry="retry" />

  <section v-if="data" class="@container min-w-0 overflow-hidden rounded-2xl border border-line bg-panel">
    <div v-if="items.length" class="flex flex-wrap items-center gap-3 border-b border-line px-5 py-3.5">
      <div class="max-w-full overflow-x-auto [scrollbar-width:none]"><Segmented v-model="source" :options="filterOptions" label="Source" /></div>
      <span class="ml-auto hidden text-[13px] text-ink3 @min-[40rem]:inline">Newest first</span>
    </div>

    <div v-for="g in groups" :key="g.day" role="group" :aria-label="g.label" class="rg [.rg+&]:border-t [.rg+&]:border-line">
      <div class="flex h-9 items-center gap-2 border-b border-line bg-panel2 px-5 text-[13px] font-medium text-ink2">
        {{ g.label }} <span class="tabular-nums text-ink3">{{ g.items.length }}</span>
      </div>
      <ul>
        <li v-for="i in g.items" :key="i.id"
          class="grid gap-x-4 gap-y-3 border-b border-line px-5 py-4 transition-colors last:border-b-0 hover:bg-panel2/60 @min-[56rem]:grid-cols-[minmax(0,1fr)_auto] @min-[56rem]:items-center">
          <div class="flex min-w-0 gap-3.5">
            <IconTile :icon="SOURCES[i.parsed.source].icon" :tone="SOURCES[i.parsed.source].tone" />
            <div class="min-w-0 flex-1">
              <p class="text-[14.5px] leading-snug font-medium wrap-anywhere">{{ i.parsed.title }}</p>
              <p v-if="i.parsed.body" class="mt-0.5 line-clamp-2 text-[13.5px] text-ink2 wrap-anywhere" :title="i.parsed.body">{{ i.parsed.body }}</p>
              <p class="mt-1 flex flex-wrap items-center gap-x-1.5 text-[12.5px] text-ink3">
                <span class="text-ink2">{{ i.parsed.from ?? SOURCES[i.parsed.source].label }}</span>
                <template v-if="i.parsed.from"><span aria-hidden="true">·</span>{{ SOURCES[i.parsed.source].label }}</template>
                <span aria-hidden="true">·</span>
                <time :datetime="i.created" :title="i.created">{{ ago(i.at, now) }}</time>
              </p>
              <p v-if="i.parsed.next" class="mt-2 inline-flex max-w-full items-start gap-1.5 rounded-lg bg-panel2 px-2.5 py-1.5 text-[13px] text-ink">
                <ArrowRight class="mt-0.5 size-3.5 shrink-0 text-ink3" aria-hidden="true" />
                <span class="wrap-anywhere"><span class="text-ink3">Suggested:</span> {{ i.parsed.next }}</span>
              </p>
            </div>
          </div>
          <div class="flex flex-wrap items-center gap-1.5 pl-[3.25rem] @min-[56rem]:pl-0">
            <AskChip :icon="Wand2" :words="`Process ${ref_(i)}: `" label="Process" />
            <AskChip :icon="ListPlus" :words="`Turn ${ref_(i)} into a task: `" label="Task" />
            <AskChip :icon="StickyNote" :words="`Save ${ref_(i)} as a note, then resolve it.`" label="Note" />
            <AskChip :icon="Check" :words="`Resolve ${ref_(i)}: nothing more to do.`" label="Done" />
          </div>
        </li>
      </ul>
    </div>

    <div v-if="!items.length" class="flex flex-col items-center gap-3 px-5 py-16 text-center">
      <span class="grid size-11 place-items-center rounded-xl bg-signal/50 text-signal-ink dark:bg-signal/15"><CheckCheck class="size-5" /></span>
      <p class="text-[15px] font-medium">Inbox zero</p>
      <p class="max-w-sm text-[13.5px] text-ink2">Capture a thought, a link or an email here, and turn it into a task or a note later.</p>
      <AskChip :icon="Plus" words="Capture in Inbox: " label="Capture something" />
    </div>
  </section>
</template>
