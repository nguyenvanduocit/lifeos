<script setup lang="ts">
// The journal as a list of days, newest first and grouped by month: each a date, the opening lines and how much was
// written, opening the day's own page. Beside it, the month as a calendar shaded by words written.
import { computed } from 'vue'
import { BookOpen, Flame, Lightbulb, NotebookPen, PenLine, Type } from '@lucide/vue'
import { useRead } from '../data'
import { num, parseDay, todayIso } from '../format'
import AskChip from '../components/AskChip.vue'
import LoadState from '../components/LoadState.vue'
import PageHeader from '../components/ds/PageHeader.vue'
import StatStrip from '../components/ds/StatStrip.vue'
import MonthCalendar from '../components/journal/MonthCalendar.vue'
import { streak } from '../components/journal/streak'

const today = todayIso()
const { data, error, loading, retry } = useRead('journal_recent', () => ({ limit: 60 }))
const entries = computed(() => [...(data.value ?? [])].sort((a, b) => b.day.localeCompare(a.day)))
const words = computed(() => new Map(entries.value.map((e) => [e.day, e.words])))
const maxWords = computed(() => Math.max(1, ...words.value.values()))
const run = computed(() => streak(words.value, today))
const avg = computed(() => (entries.value.length ? Math.round(entries.value.reduce((s, e) => s + e.words, 0) / entries.value.length) : 0))
const writtenToday = computed(() => words.value.has(today))

const monthFmt = new Intl.DateTimeFormat('en-US', { month: 'long', year: 'numeric' })
const wdFmt = new Intl.DateTimeFormat('en-US', { weekday: 'short' })
const groups = computed(() => {
  const out: { key: string; title: string; items: typeof entries.value }[] = []
  for (const e of entries.value) {
    const key = e.day.slice(0, 7)
    const last = out[out.length - 1]
    if (last?.key === key) last.items.push(e)
    else out.push({ key, title: monthFmt.format(parseDay(e.day)), items: [e] })
  }
  return out
})
const strip = computed(() => {
  if (!entries.value.length) return []
  return [
    { key: 'streak', icon: Flame, value: String(run.value), label: run.value === 1 ? 'day in a row' : 'days in a row' },
    { key: 'n', icon: BookOpen, value: String(entries.value.length), label: 'entries, last 60' },
    { key: 'avg', icon: Type, value: num(avg.value), label: 'words on average' },
  ]
})
</script>

<template>
  <PageHeader title="Journal" :eyebrow="data ? (writtenToday ? 'Written today' : 'Not written yet today') : undefined">
    <AskChip v-if="entries.length" size="sm" :icon="Lightbulb" words="Review my journal from the last two weeks and tell me what stands out." label="Reflect on two weeks" />
    <AskChip size="sm" primary :icon="PenLine" :words="writtenToday ? 'Append to today’s journal entry: ' : 'Write today’s journal entry: '" :label="writtenToday ? 'Add to today' : 'Write today'" />
    <template #strip><StatStrip v-if="strip.length" :items="strip" /></template>
  </PageHeader>

  <LoadState :loading="loading && !data" :error="error" label="journal" class="mb-4" @retry="retry" />

  <div v-if="data" class="@container">
    <div class="grid items-start gap-5 @min-[56rem]:grid-cols-[minmax(0,1fr)_19rem]">
      <section class="min-w-0 overflow-hidden rounded-2xl border border-line bg-panel">
        <RouterLink v-if="!writtenToday" :to="`/journal/${today}`"
          class="flex items-center gap-4 border-b border-dashed border-line2 px-5 py-4 text-[14px] text-ink2 hover:bg-panel2/70">
          <span class="grid size-12 shrink-0 place-items-center rounded-xl border border-dashed border-line2"><PenLine class="size-4" aria-hidden="true" /></span>
          <span class="flex-1"><span class="block font-medium text-ink">Today is still blank</span>A few lines are enough: say them to the Assistant.</span>
        </RouterLink>
        <div v-for="g in groups" :key="g.key" role="list" :aria-label="g.title">
          <div class="flex h-9 items-center gap-2 border-b border-line bg-panel2 px-5 text-[13px] font-medium text-ink2">
            {{ g.title }} <span class="text-ink3 tabular-nums">{{ g.items.length }}</span>
          </div>
          <div v-for="e in g.items" :key="e.day" role="listitem" class="relative flex gap-4 border-b border-line px-5 py-4 transition-colors last:border-b-0 hover:bg-panel2/70">
            <div class="grid w-12 shrink-0 place-items-center self-start rounded-xl py-1.5" :class="e.day === today ? 'bg-signal text-signal-fg' : 'bg-panel2'">
              <span class="text-[11px] leading-tight uppercase" :class="e.day === today ? '' : 'text-ink3'">{{ wdFmt.format(parseDay(e.day)) }}</span>
              <span class="text-[20px] leading-tight font-semibold tabular-nums">{{ parseDay(e.day).getDate() }}</span>
            </div>
            <div class="min-w-0 flex-1">
              <RouterLink :to="`/journal/${e.day}`" class="text-[14.5px] font-medium after:absolute after:inset-0 focus-visible:outline-none">
                {{ e.day === today ? 'Today' : new Intl.DateTimeFormat('en-US', { weekday: 'long', month: 'long', day: 'numeric' }).format(parseDay(e.day)) }}
              </RouterLink>
              <p v-if="e.excerpt" class="mt-0.5 line-clamp-2 text-[14px] leading-relaxed text-ink2">{{ e.excerpt }}</p>
            </div>
            <div class="hidden w-24 shrink-0 flex-col items-end gap-1.5 pt-1 @min-[36rem]:flex">
              <span class="text-[12.5px] text-ink3 tabular-nums">{{ num(e.words) }} words</span>
              <span class="h-1 w-full overflow-hidden rounded-full bg-panel3"><span class="block h-full rounded-full bg-ink/70" :style="{ width: Math.max(6, (e.words / maxWords) * 100) + '%' }" /></span>
            </div>
          </div>
        </div>
        <div v-if="!entries.length" class="flex flex-col items-center gap-3 px-5 py-14 text-center">
          <span class="grid size-11 place-items-center rounded-xl bg-panel2"><NotebookPen class="size-5 text-ink2" aria-hidden="true" /></span>
          <p class="text-[15px] font-medium">No entries yet</p>
          <AskChip :icon="PenLine" words="Write today’s journal entry: " label="Write today" />
        </div>
      </section>

      <aside class="space-y-5 @min-[56rem]:sticky @min-[56rem]:top-0">
        <MonthCalendar :words="words" />
      </aside>
    </div>
  </div>
</template>
