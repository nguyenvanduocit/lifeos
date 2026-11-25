<script setup lang="ts">
import EntityId from '../components/ds/EntityId.vue'
import PageActions from '../components/ds/PageActions.vue'
// One health metric on its own page: the latest value large with its change, its figures over the range (?range=),
// the line over the days, and each value with the change from the one before.
import { computed } from 'vue'
import {
  Activity, ArrowDownUp, CalendarDays, ChevronRight, Crosshair, MessageSquare, NotebookPen, Plus, Sigma, TrendingUp,
} from '@lucide/vue'
import { useRead } from '../data'
import { isoDay, longDay, num, parseDay, todayIso } from '../format'
import AskChip from '../components/AskChip.vue'
import Line from '../components/Line.vue'
import LoadState from '../components/LoadState.vue'
import Delta from '../components/ds/Delta.vue'
import Panel from '../components/ds/Panel.vue'
import Segmented from '../components/ds/Segmented.vue'
import StatStrip from '../components/ds/StatStrip.vue'
import TableHead from '../components/ds/TableHead.vue'
import { useQueryRef } from '../nav'
import { RANGES, rangeLabel, signed, stats } from '../components/health/stats'

const props = defineProps<{ metric: string }>()
const range = useQueryRef('range', '30', RANGES)
const days = computed(() => Number(range.value))
const { data, error, loading, retry } = useRead('metric_series', () => ({ days: days.value }))
const m = computed(() => data.value?.find((x) => x.name === props.metric))
const from = computed(() => {
  const d = parseDay(todayIso())
  d.setDate(d.getDate() - days.value)
  return isoDay(d)
})
const { data: values } = useRead('list_metrics', () => ({ name: props.metric, from: from.value }))
const rows = computed(() => {
  const sorted = [...(values.value ?? [])].sort((a, b) => a.day.localeCompare(b.day) || a.id - b.id)
  return sorted.map((r, i) => ({ ...r, change: i ? r.value - sorted[i - 1].value : null })).reverse()
})
const s = computed(() => (m.value ? stats(m.value.points) : null))
const unit = computed(() => m.value?.unit ?? '')
const strip = computed(() => {
  const x = s.value
  if (!x) return []
  const r = (n: number) => num(Math.round(n * 10) / 10)
  return [
    { key: 'avg', icon: Sigma, value: r(x.avg), label: unit.value ? `${unit.value} on average` : 'on average' },
    { key: 'lo', icon: ArrowDownUp, value: `${num(x.min)}–${num(x.max)}`, label: 'low to high' },
    { key: 'change', icon: TrendingUp, value: signed(x.change), label: `over ${rangeLabel(range.value)}` },
    { key: 'n', icon: CalendarDays, value: String(m.value!.points.length), label: 'days measured' },
  ]
})
const options = RANGES.map((r) => ({ value: r, label: rangeLabel(r) }))
const last = computed(() => m.value?.points[m.value.points.length - 1])
const ref_ = computed(() => `my ${props.metric}`)

const GRID = 'minmax(0,1.4fr) 8rem 6rem minmax(0,1.6fr)'
const cols = [
  { label: 'Day', icon: CalendarDays },
  { label: 'Value', icon: Activity },
  { label: 'Change', icon: TrendingUp, class: 'hidden @min-[44rem]:flex' },
  { label: 'Note', icon: NotebookPen, class: 'hidden @min-[44rem]:flex' },
]
</script>

<template>
  <nav aria-label="Breadcrumb" class="mb-6 flex items-center gap-1.5 text-[14px] text-ink2">
    <RouterLink to="/health" class="hover:text-ink">Health</RouterLink>
    <ChevronRight class="size-3.5 text-ink3" aria-hidden="true" />
    <span class="truncate text-ink first-letter:uppercase" aria-current="page">{{ metric }}</span>
  </nav>

  <header class="mb-6 flex flex-wrap items-end justify-between gap-x-6 gap-y-3">
    <div class="min-w-0">
      <h1 class="text-[clamp(1.75rem,3.2vw,2.4rem)] leading-[1.1] font-semibold tracking-[-0.03em] first-letter:uppercase">{{ metric }}</h1>
      <p v-if="m && m.latest !== null" class="mt-3 flex flex-wrap items-end gap-2">
        <span class="text-[44px] leading-none font-semibold tracking-[-0.03em] tabular-nums">{{ num(m.latest) }}</span>
        <span class="pb-1 text-[16px] text-ink3">{{ m.unit }}</span>
        <Delta v-if="m.previous !== null" tone="flat" class="mb-1.5 ml-1">{{ signed(m.latest - m.previous) }} vs before</Delta>
        <span v-if="last" class="mb-1.5 ml-1 text-[13.5px] text-ink2">{{ last.day === todayIso() ? 'today' : longDay(last.day) }}</span>
      </p>
    </div>
    <PageActions>
      <AskChip size="sm" :icon="MessageSquare" :words="`What does the trend in ${ref_} over the last ${rangeLabel(range)} say? `" label="Ask about the trend" />
      <AskChip size="sm" primary :icon="Plus" :words="`Log ${ref_} for today: `" label="Log today" />
    </PageActions>
  </header>

  <LoadState :loading="loading && !data" :error="error" label="metric" class="mb-4" @retry="retry" />

  <div class="@container">
    <div class="mb-5 flex flex-wrap items-center gap-3">
      <Segmented v-model="range" :options="options" label="Range" />
      <StatStrip v-if="strip.length" :items="strip" class="ml-auto" />
    </div>

    <template v-if="m">
      <Panel :title="`Over ${rangeLabel(range)}`" :icon="TrendingUp" pad>
        <template #actions><AskChip :icon="Crosshair" :words="`Set a goal for ${ref_}: `" label="Set a goal" /></template>
        <div style="--tone: var(--ink)"><Line :points="m.points" :unit="m.unit" :height="220" /></div>
      </Panel>

      <section v-if="rows.length" class="mt-5 overflow-hidden rounded-2xl border border-line bg-panel">
        <header class="flex min-h-15 items-center gap-3 px-5 py-3">
          <CalendarDays class="size-[18px]" aria-hidden="true" />
          <h2 class="text-[17px] font-semibold tracking-[-0.01em]">Values</h2>
          <span class="inline-flex h-6 items-center rounded-md border border-line bg-panel2 px-2 text-[12.5px] text-ink2 tabular-nums">{{ rows.length }}</span>
        </header>
        <div role="table" :aria-label="`${metric} values`" class="@container">
          <TableHead :grid="GRID" :cols="cols" />
          <div v-for="r in rows" :key="r.id" role="row" data-cols :style="{ '--cols': GRID, '--cols-sm': 'minmax(0,1fr) auto' }"
            class="grid items-center gap-x-4 border-b border-line px-5 py-2.5 text-[14px] last:border-b-0">
            <div role="cell" class="truncate text-ink2">{{ r.day === todayIso() ? 'Today' : longDay(r.day) }}</div>
            <div role="cell" class="tabular-nums"><span class="font-medium">{{ num(r.value) }}</span> <span class="text-ink3">{{ r.unit }}</span><EntityId class="ml-1" :value="`metric_${r.id}`" /></div>
            <div role="cell" class="hidden text-[13px] tabular-nums @min-[44rem]:block" :class="r.change ? 'text-ink2' : 'text-ink3'">{{ r.change === null ? '—' : signed(r.change) }}</div>
            <div role="cell" class="hidden min-w-0 truncate text-ink2 @min-[44rem]:block">{{ r.note ?? '' }}</div>
          </div>
        </div>
      </section>
    </template>
    <div v-else-if="data" class="rounded-2xl border border-line bg-panel px-5 py-14 text-center">
      <p class="text-[15px] font-medium">No {{ metric }} measured in the last {{ rangeLabel(range) }}.</p>
      <p class="mt-1 text-[14px] text-ink2">Try a longer range{{ data.length ? ', or pick another metric' : '' }}.</p>
      <div class="mt-3 flex flex-wrap justify-center gap-2">
        <RouterLink v-for="x in data" :key="x.name" :to="`/health/${encodeURIComponent(x.name)}`"
          class="inline-flex h-8 items-center rounded-[10px] border border-line2 px-3 text-[13px] first-letter:uppercase hover:bg-panel2">{{ x.name }}</RouterLink>
      </div>
    </div>
  </div>
</template>
