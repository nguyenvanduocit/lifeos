<script setup lang="ts">
import EntityId from '../components/ds/EntityId.vue'
import PageActions from '../components/ds/PageActions.vue'
// One habit on its own page: its streak and weeks as figures, the last twelve weeks as a heatmap, the weeks against
// the target as bars, its facts in a Details card, and what the Assistant can change about it.
import { computed } from 'vue'
import {
  Archive, CalendarCheck, CalendarDays, ChevronRight, CircleCheck, Crosshair, Flame, MessageSquare, PenLine, Ruler, Trophy,
} from '@lucide/vue'
import { useRead } from '../data'
import { shortDay, todayIso } from '../format'
import AskChip from '../components/AskChip.vue'
import Bars from '../components/Bars.vue'
import History from '../components/habits/History.vue'
import LoadState from '../components/LoadState.vue'
import IconTile from '../components/ds/IconTile.vue'
import Panel from '../components/ds/Panel.vue'
import PropertyList from '../components/ds/PropertyList.vue'
import StatStrip from '../components/ds/StatStrip.vue'
import StatusTag from '../components/ds/StatusTag.vue'
import LogButton from '../components/habits/LogButton.vue'
import { bestStreak, cadence, doneIn, weeks } from '../components/habits/math'
import { habitIcon } from '../components/planning/habitIcon'

const props = defineProps<{ name: string }>()
const today = todayIso()
const { data, error, loading, retry } = useRead('habit', () => ({ name: props.name }))
const h = data
const ref_ = computed(() => h.value?.id ?? props.name)

const best = computed(() => (h.value ? Math.max(bestStreak(h.value.recent), h.value.streak) : 0))
const byWeek = computed(() => (h.value ? weeks(h.value.recent, today).slice(-12) : []))
const strip = computed(() => {
  const x = h.value
  if (!x) return []
  return [
    { key: 'streak', icon: Flame, value: `${x.streak}d`, label: 'current streak', urgent: false },
    { key: 'week', icon: CalendarCheck, value: `${x.done_this_week}/${x.target_per_week}`, label: 'this week' },
    { key: 'month', icon: CalendarDays, value: `${doneIn(x.recent, 30)}/30`, label: 'last 30 days' },
    { key: 'best', icon: Trophy, value: `${best.value}d`, label: 'best streak, 12 weeks' },
  ]
})
const total = computed(() => (h.value?.unit ? h.value.recent.slice(-30).reduce((a, b) => a + b, 0) : null))
const rows = computed(() => {
  const x = h.value
  if (!x) return []
  return [
    { key: 'today', label: 'Today', icon: CircleCheck, value: x.done_today ? 'done' : 'open' },
    { key: 'target', label: 'Target', icon: Crosshair, value: cadence(x) },
    { key: 'unit', label: 'Counted in', icon: Ruler, value: x.unit },
    { key: 'total', label: 'Last 30 days', icon: CalendarDays, value: total.value !== null ? `${total.value} ${x.unit}` : null },
  ]
})
</script>

<template>
  <nav aria-label="Breadcrumb" class="mb-6 flex items-center gap-1.5 text-[14px] text-ink2">
    <RouterLink to="/habits" class="hover:text-ink">Habits</RouterLink>
    <ChevronRight class="size-3.5 text-ink3" aria-hidden="true" />
    <span class="truncate text-ink first-letter:uppercase" aria-current="page">{{ h?.habit ?? name }}</span>
  </nav>

  <LoadState :loading="loading && !data" :error="error" label="habit" class="mb-4" @retry="retry" />
  <div v-if="data && !h" class="rounded-2xl border border-line bg-panel px-5 py-14 text-center">
    <p class="text-[15px] font-medium">No habit called “{{ name }}”.</p>
    <RouterLink to="/habits" class="mt-2 inline-block text-[14px] text-ink2 underline underline-offset-4 hover:text-ink">Back to habits</RouterLink>
  </div>

  <div v-if="h" class="@container max-w-[1180px]">
    <header class="mb-6 flex flex-wrap items-center gap-x-4 gap-y-3">
      <IconTile :icon="habitIcon(h.habit)" tone="neutral" size="lg" />
      <div class="min-w-0 flex-1">
        <h1 class="text-[clamp(1.75rem,3.2vw,2.4rem)] leading-[1.1] font-semibold tracking-[-0.03em] first-letter:uppercase wrap-anywhere">{{ h.habit }}</h1>
        <EntityId v-if="h.id" :key="h.id" :value="h.id" />
        <p class="mt-1 text-[14px] text-ink2">{{ cadence(h) }}<template v-if="h.unit"> · counted in {{ h.unit }}</template></p>
      </div>
      <PageActions><LogButton :habit="h.habit" :done="h.done_today" size="sm" /></PageActions>
    </header>

    <StatStrip :items="strip" class="mb-6" />

    <div class="grid items-start gap-5 @min-[52rem]:grid-cols-[minmax(0,1fr)_17rem]">
      <div class="min-w-0 space-y-5">
        <Panel title="Last 12 weeks" :icon="CalendarDays" pad>
          <History :values="h.recent" :end="today" :unit="h.unit" />
        </Panel>
        <Panel title="Week by week" :icon="CalendarCheck" :count="`target ${h.target_per_week} a week`" pad>
          <div style="--tone: color-mix(in oklab, var(--ink) 70%, transparent)">
            <Bars :points="byWeek.map((w) => ({ label: 'Week of ' + shortDay(w.start), value: w.done }))" :format="(v) => `${v} ${v === 1 ? 'day' : 'days'}`"
              :reference="{ value: h.target_per_week, label: 'target' }" :height="140" />
          </div>
        </Panel>
      </div>

      <aside class="space-y-5 @min-[52rem]:sticky @min-[52rem]:top-0">
        <div class="rounded-2xl border border-line bg-panel2 px-4 py-4">
          <p class="mb-2 text-[12.5px] font-medium text-ink3">Details</p>
          <PropertyList :rows="rows" stack>
            <template #today>
              <StatusTag :tone="h.done_today ? 'good' : 'neutral'">{{ h.done_today ? 'Done today' : 'Not yet today' }}</StatusTag>
            </template>
          </PropertyList>
        </div>
        <div>
          <h3 class="mb-2.5 text-[13px] font-medium text-ink3">Ask the Assistant</h3>
          <div class="flex flex-wrap gap-2">
            <AskChip :icon="Crosshair" :words="`Change the target for the habit ${ref_} to `" label="Change target" />
            <AskChip :icon="PenLine" :words="`Rename the habit ${ref_} to `" label="Rename" />
            <AskChip :icon="MessageSquare" :words="`How am I doing with the habit ${ref_}? `" label="Ask about it" />
            <AskChip :icon="Archive" :words="`Stop tracking the habit ${ref_}.`" label="Stop tracking" />
          </div>
        </div>
      </aside>
    </div>
  </div>
</template>
