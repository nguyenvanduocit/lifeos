<script setup lang="ts">
// Habits as a list (docs/design.md, "List + peek" without the peek): one row a habit with its streak, its last four
// weeks as squares and this week against its target; the row opens the habit's own page. The filter lives in ?show=.
import { computed } from 'vue'
import { CalendarCheck, CalendarDays, CircleCheck, ClipboardCheck, Flame, Plus, Repeat, Sprout, Trophy } from '@lucide/vue'
import { useRead } from '../data'
import { useQueryRef } from '../nav'
import AskChip from '../components/AskChip.vue'
import LoadState from '../components/LoadState.vue'
import PageHeader from '../components/ds/PageHeader.vue'
import StatStrip from '../components/ds/StatStrip.vue'
import Segmented from '../components/ds/Segmented.vue'
import TableHead from '../components/ds/TableHead.vue'
import IconTile from '../components/ds/IconTile.vue'
import Squares from '../components/habits/Squares.vue'
import LogButton from '../components/habits/LogButton.vue'
import { cadence, doneIn } from '../components/habits/math'
import { habitIcon } from '../components/planning/habitIcon'

const { data, error, loading, retry } = useRead('habits')
const SHOW = ['all', 'left', 'done'] as const
const show = useQueryRef('show', 'all', SHOW)

const all = computed(() => data.value ?? [])
const left = computed(() => all.value.filter((h) => !h.done_today))
const rows = computed(() => (show.value === 'left' ? left.value : show.value === 'done' ? all.value.filter((h) => h.done_today) : all.value))
const best = computed(() => [...all.value].sort((a, b) => b.streak - a.streak)[0])
// On pace: as many days done since Monday as the target asks for by today.
const dayOfWeek = ((new Date().getDay() + 6) % 7) + 1
const onPace = (h: { done_this_week: number; target_per_week: number }) => h.done_this_week >= Math.ceil((h.target_per_week * dayOfWeek) / 7)
const onTarget = computed(() => all.value.filter(onPace).length)

const options = computed(() => [
  { value: 'all' as const, label: 'All', count: all.value.length },
  { value: 'left' as const, label: 'Left today', count: left.value.length, urgent: left.value.length > 0 },
  { value: 'done' as const, label: 'Done today', count: all.value.length - left.value.length },
])
const strip = computed(() => {
  if (!all.value.length) return []
  return [
    { key: 'today', icon: CircleCheck, value: `${all.value.length - left.value.length}/${all.value.length}`, label: 'done today', urgent: false },
    { key: 'week', icon: CalendarCheck, value: `${onTarget.value}/${all.value.length}`, label: 'on pace this week' },
    ...(best.value ? [{ key: 'best', icon: Trophy, value: `${best.value.streak}d`, label: `best streak · ${best.value.habit}`, to: `/habits/${encodeURIComponent(best.value.id ?? best.value.habit)}` }] : []),
  ]
})

const GRID = 'minmax(0,1.7fr) 7rem minmax(0,1.5fr) 7.5rem 7rem'
const cols = [
  { label: 'Habit', icon: Repeat },
  { label: 'Streak', icon: Flame, class: 'hidden @min-[44rem]:flex' },
  { label: 'Last 4 weeks', icon: CalendarDays, class: 'hidden @min-[44rem]:flex' },
  { label: 'This week', icon: CalendarCheck, class: 'hidden @min-[44rem]:flex' },
  { label: 'Today', icon: CircleCheck },
]
</script>

<template>
  <PageHeader title="Habits" :eyebrow="data ? `${all.length} tracked · ${left.length} left today` : undefined">
    <AskChip v-if="all.length" size="sm" :icon="ClipboardCheck" words="Review my habits over the last four weeks." label="Review 4 weeks" />
    <AskChip size="sm" primary :icon="Plus" words="I want to start tracking the habit " label="New habit" />
    <template #strip><StatStrip v-if="strip.length" :items="strip" /></template>
  </PageHeader>

  <LoadState :loading="loading && !data" :error="error" label="habits" class="mb-4" @retry="retry" />

  <section v-if="all.length" class="@container min-w-0 overflow-hidden rounded-2xl border border-line bg-panel">
    <div class="flex flex-wrap items-center gap-3 px-5 py-3.5">
      <Segmented v-model="show" :options="options" label="Show" />
    </div>
    <div role="table" aria-label="Habits">
      <TableHead :grid="GRID" :cols="cols" />
      <div v-for="h in rows" :key="h.habit" role="row" data-cols :style="{ '--cols': GRID, '--cols-sm': 'minmax(0,1fr) auto' }"
        class="relative grid items-center gap-x-4 border-b border-line px-5 py-3 transition-colors last:border-b-0 hover:bg-panel2/70">
        <div role="cell" class="flex min-w-0 items-center gap-3">
          <IconTile :icon="habitIcon(h.habit)" tone="neutral" />
          <div class="min-w-0">
            <RouterLink :to="`/habits/${encodeURIComponent(h.id ?? h.habit)}`" class="block truncate text-[14.5px] font-medium first-letter:uppercase after:absolute after:inset-0 focus-visible:outline-none">{{ h.habit }}</RouterLink>
            <p class="truncate text-[13px] text-ink2">
              {{ cadence(h) }}<template v-if="h.unit"> · in {{ h.unit }}</template>
              <span class="@min-[44rem]:hidden"> · {{ h.streak }}-day streak</span>
            </p>
          </div>
        </div>
        <div role="cell" class="hidden items-center gap-1.5 text-[14px] tabular-nums @min-[44rem]:flex">
          <Flame class="size-4" :class="h.streak >= 3 ? 'text-critical' : 'text-ink3'" aria-hidden="true" />
          <span class="font-medium">{{ h.streak }}</span><span class="text-ink3">{{ h.streak === 1 ? 'day' : 'days' }}</span>
        </div>
        <div role="cell" class="hidden min-w-0 items-center gap-3 @min-[44rem]:flex">
          <Squares :values="h.recent.slice(-28)" fluid />
          <span class="shrink-0 text-[12.5px] text-ink3 tabular-nums">{{ doneIn(h.recent, 28) }}/28</span>
        </div>
        <div role="cell" class="hidden items-center gap-2 @min-[44rem]:flex">
          <span class="h-1.5 w-12 overflow-hidden rounded-full bg-panel3">
            <span class="block h-full rounded-full" :class="h.done_this_week >= h.target_per_week ? 'bg-good' : onPace(h) ? 'bg-ink' : 'bg-warning'" :style="{ width: Math.min(100, (h.done_this_week / h.target_per_week) * 100) + '%' }" />
          </span>
          <span class="text-[13px] tabular-nums" :class="h.done_this_week >= h.target_per_week ? 'text-good' : onPace(h) ? 'text-ink2' : 'text-warning'" :title="onPace(h) ? 'On pace' : 'Behind pace'">{{ h.done_this_week }}/{{ h.target_per_week }}</span>
        </div>
        <div role="cell" class="flex justify-end">
          <LogButton :habit="h.habit" :done="h.done_today" />
        </div>
      </div>
      <p v-if="!rows.length" class="px-5 py-10 text-center text-[14px] text-ink2">
        {{ show === 'left' ? 'Every habit is done today.' : 'Nothing logged yet today.' }}
      </p>
    </div>
  </section>

  <div v-else-if="data" class="flex flex-col items-center gap-3 rounded-2xl border border-line bg-panel px-5 py-16 text-center">
    <span class="grid size-11 place-items-center rounded-xl bg-panel2"><Sprout class="size-5 text-ink2" aria-hidden="true" /></span>
    <p class="text-[15px] font-medium">No habits yet</p>
    <p class="max-w-sm text-[14px] text-ink2">Pick one small thing to do most days: ten minutes of reading, a walk, no sugar.</p>
    <AskChip :icon="Plus" words="I want to start tracking the habit " label="Start a habit" />
  </div>
</template>
