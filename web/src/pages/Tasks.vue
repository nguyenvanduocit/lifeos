<script setup lang="ts">
// Tasks as a list with a peek (docs/design.md, "List + peek"): views and the goal filter live in the address
// (?view=, ?goal=, ?peek=), rows sit in groups with counts, and choosing a row opens it beside the list on a wide
// screen, or on its own page on a narrow one.
import { computed } from 'vue'
import { useMediaQuery } from '@vueuse/core'
import { useRoute, useRouter } from 'vue-router'
import { AlarmClock, CalendarDays, CalendarRange, CircleCheck, Flame, ListChecks, Plus, Sparkles, Target } from '@lucide/vue'
import { useRead } from '../data'
import { useQueryRef } from '../nav'
import { longDay, relativeDay } from '../format'
import LoadState from '../components/LoadState.vue'
import AskChip from '../components/AskChip.vue'
import PageHeader from '../components/ds/PageHeader.vue'
import StatStrip from '../components/ds/StatStrip.vue'
import Segmented from '../components/ds/Segmented.vue'
import TableHead from '../components/ds/TableHead.vue'
import TaskLine from '../components/tasks/TaskLine.vue'
import TaskDetail from '../components/tasks/TaskDetail.vue'
import { dayOf } from '../components/tasks/when'
import type { DocSummary } from '../core/generated/DocSummary'

const route = useRoute()
const router = useRouter()
const VIEWS = ['today', 'upcoming', 'anytime', 'done'] as const
const view = useQueryRef('view', 'today', VIEWS)
const goalFilter = useQueryRef('goal', '')
const peek = computed(() => (typeof route.query.peek === 'string' ? route.query.peek : ''))
const wide = useMediaQuery('(min-width: 64rem)')

const { data: v, error, loading, retry } = useRead('tasks_view')
const { data: doneList, loading: doneLoading, error: doneError, retry: retryDone } = useRead('list_tasks', () => ({ status: view.value === 'done' ? 'done' : 'open' }))
const { data: goals } = useRead('list_goals', () => ({}))
const goalTitle = computed(() => new Map((goals.value ?? []).map((g) => [g.summary.id, g.summary.title])))

const keep = (t: DocSummary) => !goalFilter.value || t.goal === goalFilter.value
const groups = computed((): { key: string; title: string; urgent?: boolean; tasks: DocSummary[]; done?: boolean }[] => {
  const x = v.value
  if (!x) return []
  if (view.value === 'today') {
    return [
      { key: 'overdue', title: 'Overdue', urgent: true, tasks: x.overdue.filter(keep) },
      { key: 'today', title: 'Today', tasks: x.today.filter(keep) },
      { key: 'done', title: 'Done today', tasks: x.done_today.filter(keep), done: true },
    ].filter((g) => g.tasks.length)
  }
  if (view.value === 'upcoming') {
    const byDay = new Map<string, DocSummary[]>()
    for (const t of x.upcoming.filter(keep)) {
      const d = dayOf(t) ?? ''
      byDay.set(d, [...(byDay.get(d) ?? []), t])
    }
    return [...byDay.entries()].sort(([a], [b]) => a.localeCompare(b)).map(([d, tasks]) => ({ key: d, title: `${relativeDay(d)} · ${longDay(d)}`, tasks }))
  }
  if (view.value === 'anytime') return [{ key: 'anytime', title: 'No date', tasks: x.anytime.filter(keep) }].filter((g) => g.tasks.length)
  return [{ key: 'done', title: 'Completed', tasks: (doneList.value ?? []).filter(keep), done: true }].filter((g) => g.tasks.length)
})
const counts = computed(() => ({
  today: (v.value?.overdue.length ?? 0) + (v.value?.today.length ?? 0),
  upcoming: v.value?.upcoming.length ?? 0,
  anytime: v.value?.anytime.length ?? 0,
}))
const viewOptions = computed(() => [
  { value: 'today' as const, label: 'Today', count: counts.value.today, urgent: !!v.value?.overdue.length },
  { value: 'upcoming' as const, label: 'Upcoming', count: counts.value.upcoming },
  { value: 'anytime' as const, label: 'Anytime', count: counts.value.anytime },
  { value: 'done' as const, label: 'Done' },
])
const strip = computed(() => {
  const x = v.value
  if (!x) return []
  return [
    { key: 'late', icon: AlarmClock, value: String(x.overdue.length), label: 'overdue', urgent: x.overdue.length > 0 },
    { key: 'today', icon: CalendarDays, value: String(x.today.length), label: 'for today' },
    { key: 'week', icon: CalendarRange, value: String(x.upcoming.length), label: 'in the next 7 days' },
    { key: 'done', icon: CircleCheck, value: String(x.done_today.length), label: 'done today' },
  ]
})

const withGoal = computed(() => groups.value.some((g) => g.tasks.some((t) => t.goal)))
const GRID = computed(() => (withGoal.value ? 'minmax(0,2.4fr) minmax(0,1.2fr) minmax(0,0.9fr) minmax(0,0.6fr)' : 'minmax(0,3fr) minmax(0,1fr) minmax(0,0.6fr)'))
const cols = computed(() => [
  { label: 'Task', icon: ListChecks },
  ...(withGoal.value ? [{ label: 'Goal', icon: Target, class: 'hidden @min-[44rem]:flex' }] : []),
  { label: 'When', icon: CalendarDays },
  { label: 'Priority', icon: Flame, class: 'hidden @min-[44rem]:flex' },
])
const rowLink = (id: string) => (wide.value ? { query: { ...route.query, peek: id } } : `/tasks/${id}`)
function closePeek() {
  void router.replace({ query: { ...route.query, peek: undefined } })
}
const empty = computed(() => ({
  today: 'Nothing due or planned today.',
  upcoming: 'Nothing planned in the next 7 days.',
  anytime: 'Every task has a date.',
  done: 'No completed tasks yet.',
}[view.value]))
</script>

<template>
  <PageHeader title="Tasks" :eyebrow="v ? `${counts.today + counts.upcoming + counts.anytime} open · ${v.overdue.length} overdue` : undefined">
    <AskChip size="sm" :icon="Sparkles" words="Reschedule tasks I cannot finish this week." label="Plan the week" />
    <AskChip size="sm" primary :icon="Plus" words="Add a task: " label="Add task" />
    <template #strip><StatStrip v-if="strip.length" :items="strip" /></template>
  </PageHeader>

  <div class="grid items-start gap-5" :class="peek && wide ? 'grid-cols-[minmax(0,1fr)_420px]' : ''">
    <section class="@container min-w-0 overflow-hidden rounded-2xl border border-line bg-panel">
      <div class="flex flex-wrap items-center gap-3 px-5 py-3.5">
        <Segmented v-model="view" :options="viewOptions" label="View" />
        <label class="ml-auto flex items-center gap-2 text-[13px] text-ink2">
          <Target class="size-4 text-ink3" aria-hidden="true" />
          <span class="sr-only">Goal</span>
          <select v-model="goalFilter" class="h-8 max-w-48 cursor-pointer appearance-none rounded-[10px] border border-line2 bg-panel bg-[length:14px] bg-[right_8px_center] bg-no-repeat pr-7 pl-2.5 text-[13px] text-ink hover:bg-panel2 focus-visible:outline-2 focus-visible:outline-ring"
            style="background-image: url(&quot;data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='%23888' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'%3E%3Cpath d='m6 9 6 6 6-6'/%3E%3C/svg%3E&quot;)">
            <option value="">All goals</option>
            <option v-for="g in goals ?? []" :key="g.summary.id" :value="g.summary.id">{{ g.summary.title }}</option>
          </select>
        </label>
      </div>

      <LoadState :loading="(loading && !v) || (view === 'done' && doneLoading && !doneList)" :error="error || (view === 'done' ? doneError : '')" label="tasks" class="px-5 pb-4" @retry="view === 'done' ? retryDone() : retry()" />
      <div v-if="v" role="table" aria-label="Tasks">
        <TableHead :grid="GRID" :cols="cols" />
        <template v-for="g in groups" :key="g.key">
          <div role="rowgroup" class="rg [.rg+&]:border-t [.rg+&]:border-line">
            <div class="flex h-9 items-center gap-2 border-b border-line bg-panel px-5 text-[13px] font-medium" :class="g.urgent ? 'text-critical' : 'text-ink2'">
              {{ g.title }} <span class="tabular-nums text-ink3">{{ g.tasks.length }}</span>
            </div>
            <TaskLine v-for="t in g.tasks" :key="t.id" :task="t" :done="g.done" :goal="t.goal ? goalTitle.get(t.goal) : undefined" :show-goal="withGoal"
              :grid="GRID" :to="rowLink(t.id)" :selected="peek === t.id" />
          </div>
        </template>
        <div v-if="!groups.length" class="flex flex-col items-center gap-3 px-5 py-14 text-center">
          <span class="grid size-11 place-items-center rounded-xl bg-panel2"><CircleCheck class="size-5 text-ink2" /></span>
          <p class="text-[15px] font-medium">{{ empty }}</p>
          <AskChip :icon="Plus" words="Add a task: " label="Add task" />
        </div>
      </div>
    </section>

    <aside v-if="peek && wide" class="sticky top-0 h-[calc(100dvh-8.5rem)] overflow-hidden rounded-2xl border border-line bg-panel shadow-xl shadow-black/5" aria-label="Task">
      <TaskDetail :id="peek" :key="peek" @close="closePeek" />
    </aside>
  </div>
</template>
