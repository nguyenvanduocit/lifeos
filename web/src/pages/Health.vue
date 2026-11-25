<script setup lang="ts">
// Health: one tile a metric (its latest value, the change from the one before, its range and recent days as bars),
// each opening the metric's own page; below, the latest measurements as a table. The range lives in ?range=.
import { computed } from 'vue'
import { Activity, CalendarDays, Gauge, HeartPulse, Moon, NotebookPen, Plus, Scale, Smile } from '@lucide/vue'
import type { Component } from 'vue'
import { useRead } from '../data'
import { useQueryRef } from '../nav'
import { isoDay, num, parseDay, shortDay, todayIso } from '../format'
import AskChip from '../components/AskChip.vue'
import LoadState from '../components/LoadState.vue'
import PageHeader from '../components/ds/PageHeader.vue'
import Segmented from '../components/ds/Segmented.vue'
import TableHead from '../components/ds/TableHead.vue'
import IconTile from '../components/ds/IconTile.vue'
import Delta from '../components/ds/Delta.vue'
import Spark from '../components/health/Spark.vue'
import { RANGES, rangeLabel, signed, stats } from '../components/health/stats'

const range = useQueryRef('range', '30', RANGES)
const days = computed(() => Number(range.value))
const { data, error, loading, retry } = useRead('metric_series', () => ({ days: days.value }))
const from = computed(() => {
  const d = parseDay(todayIso())
  d.setDate(d.getDate() - days.value)
  return isoDay(d)
})
const { data: recent } = useRead('list_metrics', () => ({ from: from.value }))
const latest = computed(() => [...(recent.value ?? [])].sort((a, b) => b.day.localeCompare(a.day) || b.id - a.id).slice(0, 12))

const ICONS: [RegExp, Component][] = [[/sleep|ngủ/i, Moon], [/weight|cân/i, Scale], [/mood|tâm trạng/i, Smile], [/heart|pulse|hr|tim/i, HeartPulse], [/step|bước/i, Activity]]
const iconOf = (name: string) => ICONS.find(([re]) => re.test(name))?.[1] ?? Gauge
const link = (name: string) => `/health/${encodeURIComponent(name)}`
const options = RANGES.map((r) => ({ value: r, label: rangeLabel(r) }))
const lastDay = computed(() => latest.value[0]?.day)

const GRID = '8rem minmax(0,1fr) 8rem minmax(0,1.4fr)'
const cols = [
  { label: 'Day', icon: CalendarDays },
  { label: 'Metric', icon: Gauge, class: 'hidden @min-[44rem]:flex' },
  { label: 'Value', icon: Activity },
  { label: 'Note', icon: NotebookPen, class: 'hidden @min-[44rem]:flex' },
]
</script>

<template>
  <PageHeader title="Health" :eyebrow="data ? `${data.length} metrics${lastDay ? ' · last measured ' + (lastDay === todayIso() ? 'today' : shortDay(lastDay)) : ''}` : undefined">
    <AskChip size="sm" :icon="Moon" words="Last night I slept for " label="Log sleep" />
    <AskChip size="sm" :icon="Smile" words="My mood today (1–10): " label="Log mood" />
    <AskChip size="sm" primary :icon="Plus" words="Log a health measurement: " label="Log measurement" />
  </PageHeader>

  <div class="mb-4 flex flex-wrap items-center gap-3">
    <Segmented v-model="range" :options="options" label="Range" />
  </div>

  <LoadState :loading="loading && !data" :error="error" label="health metrics" class="mb-4" @retry="retry" />

  <div v-if="data?.length" class="@container">
    <ul class="grid gap-4 @min-[40rem]:grid-cols-2 @min-[64rem]:grid-cols-3">
      <li v-for="m in data" :key="m.name" class="relative flex flex-col gap-4 rounded-2xl border border-line bg-panel p-5 transition-colors hover:border-line2 hover:bg-panel2/40">
        <div class="flex items-center gap-3">
          <IconTile :icon="iconOf(m.name)" tone="neutral" size="sm" />
          <RouterLink :to="link(m.name)" class="flex-1 truncate text-[15px] font-medium first-letter:uppercase after:absolute after:inset-0 focus-visible:outline-none">{{ m.name }}</RouterLink>
          <span class="text-[12.5px] text-ink3 tabular-nums">{{ m.points.length }} measured</span>
        </div>
        <div class="flex items-end gap-2">
          <span class="text-[32px] leading-none font-semibold tracking-[-0.02em] tabular-nums">{{ m.latest !== null ? num(m.latest) : '–' }}</span>
          <span class="pb-0.5 text-[14px] text-ink3">{{ m.unit }}</span>
          <Delta v-if="m.latest !== null && m.previous !== null" tone="flat" class="mb-1 ml-1">{{ signed(m.latest - m.previous) }}</Delta>
        </div>
        <Spark v-if="m.points.length" :points="m.points" :count="days <= 30 ? days : 40" height="h-12" />
        <p v-if="stats(m.points)" class="text-[13px] text-ink2">
          avg <span class="text-ink tabular-nums">{{ num(Math.round(stats(m.points)!.avg * 10) / 10) }}</span>
          · range <span class="text-ink tabular-nums">{{ num(stats(m.points)!.min) }}–{{ num(stats(m.points)!.max) }}</span>
          <span class="text-ink3"> · {{ rangeLabel(range) }}</span>
        </p>
      </li>
    </ul>

    <section v-if="latest.length" class="mt-5 overflow-hidden rounded-2xl border border-line bg-panel">
      <header class="flex min-h-15 items-center gap-3 px-5 py-3">
        <NotebookPen class="size-[18px] text-ink" aria-hidden="true" />
        <h2 class="text-[17px] font-semibold tracking-[-0.01em]">Latest measurements</h2>
      </header>
      <div role="table" aria-label="Latest measurements" class="@container">
        <TableHead :grid="GRID" :cols="cols" />
        <div v-for="r in latest" :key="r.id" role="row" data-cols :style="{ '--cols': GRID, '--cols-sm': 'minmax(0,1fr) auto' }"
          class="relative grid items-center gap-x-4 border-b border-line px-5 py-3 text-[14px] last:border-b-0 hover:bg-panel2/70">
          <div role="cell" class="min-w-0 text-ink2">
            {{ r.day === todayIso() ? 'Today' : shortDay(r.day) }}
            <RouterLink :to="link(r.name)" class="ml-1 inline-block text-ink first-letter:uppercase after:absolute after:inset-0 @min-[44rem]:hidden">{{ r.name }}</RouterLink>
          </div>
          <div role="cell" class="hidden min-w-0 @min-[44rem]:block">
            <RouterLink :to="link(r.name)" class="block truncate font-medium first-letter:uppercase after:absolute after:inset-0">{{ r.name }}</RouterLink>
          </div>
          <div role="cell" class="tabular-nums"><span class="font-medium">{{ num(r.value) }}</span> <span class="text-ink3">{{ r.unit }}</span></div>
          <div role="cell" class="hidden min-w-0 truncate text-ink2 @min-[44rem]:block">{{ r.note ?? '' }}</div>
        </div>
      </div>
    </section>
  </div>

  <div v-else-if="data" class="flex flex-col items-center gap-3 rounded-2xl border border-line bg-panel px-5 py-16 text-center">
    <span class="grid size-11 place-items-center rounded-xl bg-panel2"><HeartPulse class="size-5 text-ink2" aria-hidden="true" /></span>
    <p class="text-[15px] font-medium">No measurements in the last {{ rangeLabel(range) }}</p>
    <p class="max-w-sm text-[14px] text-ink2">Tell the Assistant your weight, sleep or mood, or connect the iPhone app for Apple Health.</p>
    <AskChip :icon="Plus" words="Log a health measurement: " label="Log measurement" />
  </div>
</template>
