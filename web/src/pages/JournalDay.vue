<script setup lang="ts">
import PageActions from '../components/ds/PageActions.vue'
// One day of the journal on its own page: the date, the way to the entry before and after, the entry in a reading
// column, and beside it the month and what the Assistant can do with the day.
import { computed } from 'vue'
import { ChevronLeft, ChevronRight, Clock, FileText, NotebookPen, PenLine, Sparkles, Type } from '@lucide/vue'
import { useRead } from '../data'
import { isoDay, isoWeek, longDay, num, parseDay, todayIso } from '../format'
import AskChip from '../components/AskChip.vue'
import LoadState from '../components/LoadState.vue'
import Markdown from '../components/Markdown.vue'
import PropertyList from '../components/ds/PropertyList.vue'
import MonthCalendar from '../components/journal/MonthCalendar.vue'

const props = defineProps<{ day: string }>()
const today = todayIso()
const valid = computed(() => /^\d{4}-\d{2}-\d{2}$/.test(props.day))
const { data: page, loading, error, retry } = useRead('read_journal', () => ({ date: props.day }))
const { data: recent } = useRead('journal_recent', () => ({ limit: 60 }))
const words = computed(() => new Map((recent.value ?? []).map((e) => [e.day, e.words])))
const sorted = computed(() => [...words.value.keys()].sort())
const older = computed(() => [...sorted.value].reverse().find((d) => d < props.day))
const newer = computed(() => sorted.value.find((d) => d > props.day) ?? (props.day < today ? today : undefined))
const isToday = computed(() => props.day === today)

const shift = (by: number) => {
  const d = parseDay(props.day)
  d.setDate(d.getDate() + by)
  return isoDay(d)
}
const bodyWords = computed(() => (page.value?.body.trim() ? page.value.body.trim().split(/\s+/u).length : 0))
const rows = computed(() => [
  { key: 'words', label: 'Words', icon: Type, value: bodyWords.value ? num(bodyWords.value) : null },
  { key: 'read', label: 'Reading time', icon: Clock, value: bodyWords.value ? `${Math.max(1, Math.round(bodyWords.value / 200))} min` : null },
  { key: 'path', label: 'File', icon: FileText, value: page.value?.summary.path },
])
const about = computed(() => (isToday.value ? 'today’s journal entry' : `the journal entry for ${props.day}`))
</script>

<template>
  <nav aria-label="Breadcrumb" class="mb-6 flex items-center gap-1.5 text-[14px] text-ink2">
    <RouterLink to="/journal" class="hover:text-ink">Journal</RouterLink>
    <ChevronRight class="size-3.5 text-ink3" aria-hidden="true" />
    <span class="truncate text-ink" aria-current="page">{{ valid ? longDay(day) : day }}</span>
  </nav>

  <div v-if="!valid" class="rounded-2xl border border-line bg-panel px-5 py-14 text-center text-[15px]">“{{ day }}” is not a day. Days look like {{ today }}.</div>

  <div v-else class="@container max-w-[1180px]">
    <header class="mb-6 flex flex-wrap items-end justify-between gap-x-6 gap-y-3">
      <div class="min-w-0">
        <p class="mb-1 text-[14px] text-ink2">{{ isToday ? 'Today' : parseDay(day).getFullYear() }} · Week {{ isoWeek(day) }}</p>
        <h1 class="text-[clamp(1.75rem,3.2vw,2.4rem)] leading-[1.1] font-semibold tracking-[-0.03em]">{{ longDay(day) }}</h1>
      </div>
      <PageActions>
        <RouterLink :to="`/journal/${older ?? shift(-1)}`" class="inline-flex h-8 items-center gap-1.5 rounded-lg border border-line2 bg-panel px-2.5 text-[13px] font-medium hover:bg-panel2"
          :title="older ? 'Older entry: ' + longDay(older) : 'The day before'">
          <ChevronLeft class="size-3.5" aria-hidden="true" />{{ older ? 'Older' : 'Day before' }}
        </RouterLink>
        <RouterLink v-if="newer" :to="`/journal/${newer}`" class="inline-flex h-8 items-center gap-1.5 rounded-lg border border-line2 bg-panel px-2.5 text-[13px] font-medium hover:bg-panel2"
          :title="'Newer entry: ' + longDay(newer)">
          {{ newer === today ? 'Today' : 'Newer' }}<ChevronRight class="size-3.5" aria-hidden="true" />
        </RouterLink>
      </PageActions>
    </header>

    <div class="grid items-start gap-5 @min-[56rem]:grid-cols-[minmax(0,1fr)_19rem]">
      <article class="min-w-0 rounded-2xl border border-line bg-panel px-6 py-7 sm:px-10 sm:py-9" :aria-label="'Journal, ' + longDay(day)">
        <LoadState v-if="loading || error" :loading="loading" :error="error" label="journal entry" @retry="retry" />
        <Markdown v-else-if="page?.body.trim()" :source="page.body" class="max-w-[68ch] text-[16px] leading-[1.75]" />
        <div v-else class="flex flex-col items-center gap-3 py-10 text-center">
          <span class="grid size-11 place-items-center rounded-xl bg-panel2"><NotebookPen class="size-5 text-ink2" aria-hidden="true" /></span>
          <p class="text-[15px] font-medium">{{ isToday ? 'A blank page' : 'Nothing written this day' }}</p>
          <p class="max-w-sm text-[14px] text-ink2">{{ isToday ? 'How was today? A few lines are enough; say them to the Assistant and they land here.' : `You can still add a few lines about ${longDay(day)}.` }}</p>
          <AskChip primary :icon="PenLine" :words="isToday ? 'Write today’s journal entry: ' : `Write the journal entry for ${day}: `" :label="isToday ? 'Write today' : 'Write for this day'" />
        </div>
      </article>

      <aside class="space-y-5 @min-[56rem]:sticky @min-[56rem]:top-0">
        <div v-if="page?.body.trim()" class="rounded-2xl border border-line bg-panel2 px-4 py-4">
          <p class="mb-2 text-[12.5px] font-medium text-ink3">Details</p>
          <PropertyList :rows="rows" stack>
            <template #path><span class="truncate font-mono text-[12px] text-ink3">{{ page?.summary.path }}</span></template>
          </PropertyList>
        </div>
        <div v-if="page?.body.trim()">
          <h3 class="mb-2.5 text-[13px] font-medium text-ink3">Ask the Assistant</h3>
          <div class="flex flex-wrap gap-2">
            <AskChip :icon="PenLine" :words="`Append to ${about}: `" label="Add to this entry" />
            <AskChip :icon="Sparkles" :words="`Read ${about} and reflect it back to me: what stands out, and what I might be missing.`" label="Reflect on it" />
          </div>
        </div>
        <MonthCalendar :words="words" :current="day" />
      </aside>
    </div>
  </div>
</template>
