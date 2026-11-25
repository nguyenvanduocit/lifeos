<script setup lang="ts">
// Goals as a table (docs/design.md, "List"): each goal's progress, its tasks and its horizon, active first. The state
// filter lives in the address (?status=); a row opens the goal's own page.
import { computed } from 'vue'
import { CalendarRange, CircleDot, ClipboardCheck, Gauge, ListChecks, Plus, Target, Trophy, Mountain } from '@lucide/vue'
import { useRead } from '../data'
import { useQueryRef } from '../nav'
import AskChip from '../components/AskChip.vue'
import LoadState from '../components/LoadState.vue'
import PageHeader from '../components/ds/PageHeader.vue'
import StatStrip from '../components/ds/StatStrip.vue'
import Segmented from '../components/ds/Segmented.vue'
import TableHead from '../components/ds/TableHead.vue'
import GoalLine from '../components/goals/GoalLine.vue'

const { data, error, loading, retry } = useRead('list_goals', () => ({}))
const VIEWS = ['active', 'paused', 'done', 'dropped', 'all'] as const
const view = useQueryRef('status', 'active', VIEWS)

const count = (s: string) => (data.value ?? []).filter((g) => g.summary.status === s).length
const shown = computed(() => (data.value ?? [])
  .filter((g) => view.value === 'all' || g.summary.status === view.value)
  .sort((a, b) => (b.summary.progress ?? 0) - (a.summary.progress ?? 0)))
const options = computed(() => [
  { value: 'active' as const, label: 'Active', count: count('active') },
  { value: 'paused' as const, label: 'Paused', count: count('paused') },
  { value: 'done' as const, label: 'Achieved', count: count('done') },
  { value: 'dropped' as const, label: 'Dropped', count: count('dropped') },
  { value: 'all' as const, label: 'All' },
])

const strip = computed(() => {
  const active = (data.value ?? []).filter((g) => g.summary.status === 'active')
  const average = active.length ? Math.round(active.reduce((sum, g) => sum + (g.summary.progress ?? 0), 0) / active.length) : 0
  const open = active.reduce((sum, g) => sum + g.open_tasks, 0)
  const stalled = active.filter((g) => g.open_tasks === 0 && (g.summary.progress ?? 0) < 100).length
  return [
    { key: 'active', icon: Target, value: String(active.length), label: 'active goals' },
    { key: 'avg', icon: Gauge, value: average + '%', label: 'average progress' },
    { key: 'open', icon: ListChecks, value: String(open), label: 'open tasks toward them', delta: stalled ? `${stalled} with no next task` : undefined, deltaTone: 'bad' as const },
    { key: 'done', icon: Trophy, value: String(count('done')), label: 'achieved' },
  ]
})

const GRID = 'minmax(0,2.6fr) minmax(0,1.1fr) minmax(0,1fr) minmax(0,0.7fr) minmax(0,0.8fr)'
const cols = [
  { label: 'Goal', icon: Target },
  { label: 'Progress', icon: Gauge },
  { label: 'Tasks', icon: ListChecks, class: 'hidden @min-[44rem]:flex' },
  { label: 'Horizon', icon: CalendarRange, class: 'hidden @min-[44rem]:flex' },
  { label: 'Status', icon: CircleDot, class: 'hidden @min-[44rem]:flex' },
]
const empty = computed(() => ({
  active: 'No active goals',
  paused: 'No paused goals',
  done: 'No goals achieved yet',
  dropped: 'No dropped goals',
  all: 'No goals yet',
}[view.value]))
</script>

<template>
  <PageHeader title="Goals">
    <AskChip v-if="data?.length" size="sm" :icon="ClipboardCheck" words="Review my goals and update their progress." label="Review progress" />
    <AskChip size="sm" primary :icon="Plus" words="I want to set a goal: " label="New goal" />
    <template v-if="data?.length" #strip><StatStrip :items="strip" /></template>
  </PageHeader>
  <LoadState :loading="loading && !data" :error="error" label="goals" class="mb-4" @retry="retry" />

  <section v-if="data" class="@container min-w-0 overflow-hidden rounded-2xl border border-line bg-panel">
    <div class="flex flex-wrap items-center gap-3 px-5 py-3">
      <div class="max-w-full overflow-x-auto [scrollbar-width:none]">
        <Segmented v-model="view" :options="options" label="State" />
      </div>
    </div>
    <div role="table" aria-label="Goals">
      <TableHead :grid="GRID" :cols="cols" />
      <GoalLine v-for="g in shown" :key="g.summary.id" :goal="g" :grid="GRID" />
    </div>
    <div v-if="!shown.length" class="flex flex-col items-center gap-3 px-5 py-14 text-center">
      <span class="grid size-11 place-items-center rounded-xl bg-panel2"><Mountain class="size-5 text-ink2" /></span>
      <p class="text-[15px] font-medium">{{ empty }}</p>
      <p v-if="view === 'active'" class="max-w-sm text-[13.5px] text-ink2">A goal gives your tasks a direction. Start with one or two.</p>
      <AskChip :icon="Target" words="Help me set a goal: " label="Set a goal" />
    </div>
  </section>
</template>
