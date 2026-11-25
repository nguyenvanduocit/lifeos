<script setup lang="ts">
// What runs by itself, in four views kept in the address (?tab=): routines (an agent put to work on a schedule, once,
// from a webhook or after a change), their runs (?routine= narrows them), workflow scripts and their runs, and the
// hand-offs still out between agents. Nothing here acts: the chips draft a word to the Assistant.
import { computed } from 'vue'
import {
  AlarmClock, CalendarCheck, CalendarClock, Clock, FileCode2, History, ListFilter, MessagesSquare, Play, Plus, Sunrise, Workflow, XCircle,
  Activity, CalendarDays,
} from '@lucide/vue'
import { ASSISTANT } from '../assistant'
import { useRead } from '../data'
import { useQueryRef } from '../nav'
import AskChip from '../components/AskChip.vue'
import LoadState from '../components/LoadState.vue'
import PageHeader from '../components/ds/PageHeader.vue'
import StatStrip from '../components/ds/StatStrip.vue'
import Segmented from '../components/ds/Segmented.vue'
import TableHead from '../components/ds/TableHead.vue'
import RoutineRow from '../components/automation/RoutineRow.vue'
import SignalView from '../components/automation/SignalView.vue'
import RunRow from '../components/automation/RunRow.vue'
import WorkflowRunRow from '../components/automation/WorkflowRunRow.vue'
import HandoffLine from '../components/crew/HandoffLine.vue'
import { ago, when } from '../components/agents/when'

const TABS = ['routines', 'runs', 'workflows', 'signals', 'handoffs'] as const
const tab = useQueryRef('tab', 'routines', TABS)
const routine = useQueryRef<string>('routine', '')

const { data: routines, error: routinesError, loading: routinesLoading, retry: retryRoutines } = useRead('list_routines')
const { data: runs, error: runsError, loading: runsLoading, retry: retryRuns } = useRead('list_runs', () => ({ routine: routine.value || undefined, limit: 60 }))
const { data: flows, error: flowsError, loading: flowsLoading, retry: retryFlows } = useRead('list_workflows', () => ({ runs: true }))
const { data: out } = useRead('list_handoffs', () => ({ open: true }))
const { data: crew } = useRead('list_crew')
const names = computed(() => new Map([[ASSISTANT, 'Assistant'], ...(crew.value ?? []).map((m) => [m.id, m.name] as const)]))
const nameOf = (id: string) => names.value.get(id) ?? id

function showRuns(id: string) {
  routine.value = id
  tab.value = 'runs'
}

const today = new Date().toDateString()
const stats = computed(() => {
  const list = routines.value ?? []
  const all = runs.value ?? []
  const nexts = list.filter((r) => r.status === 'active' && r.next).map((r) => r.next as string).sort()
  return {
    active: list.filter((r) => r.status === 'active').length,
    paused: list.filter((r) => r.status === 'paused').length,
    today: all.filter((r) => new Date(r.queued.replace(' ', 'T')).toDateString() === today).length,
    failed: all.filter((r) => r.error).length,
    next: nexts[0],
  }
})
const strip = computed(() => [
  { key: 'active', icon: Play, value: String(stats.value.active), label: stats.value.paused ? `active · ${stats.value.paused} paused` : 'active routines' },
  { key: 'next', icon: AlarmClock, value: stats.value.next ? ago(stats.value.next) : '—', label: stats.value.next ? `next run · ${when(stats.value.next)}` : 'nothing scheduled' },
  { key: 'today', icon: History, value: String(stats.value.today), label: 'runs today', to: '/automation?tab=runs' },
  { key: 'failed', icon: XCircle, value: String(stats.value.failed), label: 'failed of the last runs', urgent: stats.value.failed > 0 },
])

const tabs = computed(() => [
  { value: 'routines' as const, label: 'Routines', count: routines.value?.length },
  { value: 'runs' as const, label: 'Runs', count: runs.value?.length },
  { value: 'workflows' as const, label: 'Workflows', count: flows.value?.runs.length },
  { value: 'signals' as const, label: 'Signals' },
  { value: 'handoffs' as const, label: 'Hand-offs out', count: out.value?.length, urgent: (out.value ?? []).some((h) => h.note) },
])

const RUN_GRID = 'minmax(0,2.4fr) minmax(0,1.2fr) minmax(0,0.8fr) 7.5rem'
const RUN_GRID_SM = 'minmax(0,1fr) 7.5rem'
const runCols = [
  { label: 'Run', icon: Activity },
  { label: 'Trigger', icon: ListFilter, class: 'hidden @min-[44rem]:flex' },
  { label: 'When', icon: CalendarDays, class: 'hidden @min-[44rem]:flex' },
  { label: 'Status', icon: Clock },
]

const STARTERS = [
  { label: 'Weekly task summary', icon: ListFilter, words: 'Create a routine for Assistant to summarize incomplete tasks every Friday at 5:00 PM.' },
  { label: 'Morning inbox sweep', icon: Sunrise, words: 'Create a routine that every weekday at 07:30 reads my new email and puts what needs me into the LifeOS Inbox.' },
  { label: 'Sunday review', icon: CalendarCheck, words: 'Create a routine that every Sunday at 20:00 drafts my weekly review.' },
]
</script>

<template>
  <PageHeader title="Automation" :eyebrow="routines ? `${routines.length} routine${routines.length === 1 ? '' : 's'} · ${flows?.scripts.length ?? 0} workflow scripts` : undefined">
    <AskChip size="sm" :icon="Workflow" words="Run the workflow " label="Run a workflow" />
    <AskChip size="sm" primary :icon="Plus" label="New routine" words="Create a routine with this schedule and instruction: " />
    <template v-if="routines?.length" #strip><StatStrip :items="strip" /></template>
  </PageHeader>

  <section class="@container min-w-0 overflow-hidden rounded-2xl border border-line bg-panel">
    <div class="flex flex-wrap items-center gap-3 px-5 py-3.5">
      <Segmented v-model="tab" :options="tabs" label="View" />
      <label v-if="tab === 'runs'" class="ml-auto flex items-center gap-2 text-[13px] text-ink2">
        <CalendarClock class="size-4 text-ink3" aria-hidden="true" /><span class="sr-only">Routine</span>
        <select v-model="routine" class="lf-select h-8 max-w-52 cursor-pointer appearance-none rounded-[10px] border border-line2 bg-panel pr-7 pl-2.5 text-[13px] text-ink hover:bg-panel2">
          <option value="">All routines</option>
          <option v-for="r in routines ?? []" :key="r.id" :value="r.id">{{ r.title }}</option>
        </select>
      </label>
    </div>

    <SignalView v-if="tab === 'signals'" :routines="routines" :routines-error="routinesError" />
    <template v-else-if="tab === 'routines'">
      <LoadState v-if="routinesError || !routines" :loading="routinesLoading" :error="routinesError" label="routines" class="px-5 pb-4" @retry="retryRoutines" />
      <ul v-else-if="routines.length" class="border-t border-line">
        <RoutineRow v-for="r in routines" :key="r.id" :routine="r" :agent-name="nameOf(r.agent)" @runs="showRuns" />
      </ul>
      <div v-else class="flex flex-col items-center gap-3 border-t border-line px-5 py-14 text-center">
        <span class="grid size-11 place-items-center rounded-xl bg-panel2"><CalendarClock class="size-5 text-ink2" /></span>
        <p class="text-[15px] font-medium">No routines yet</p>
        <p class="max-w-md text-[13.5px] text-ink2">A routine puts an agent to work by itself: on a schedule, once at a time, from a webhook, or after something changes in LifeOS.</p>
        <div class="flex flex-wrap justify-center gap-2">
          <AskChip v-for="s in STARTERS" :key="s.label" :icon="s.icon" :label="s.label" :words="s.words" />
        </div>
      </div>
    </template>

    <!-- Runs -->
    <template v-else-if="tab === 'runs'">
      <LoadState v-if="runsError || !runs" :loading="runsLoading" :error="runsError" label="run history" class="px-5 pb-4" @retry="retryRuns" />
      <div v-else-if="runs.length" role="table" aria-label="Runs">
        <TableHead :grid="RUN_GRID" :grid-sm="RUN_GRID_SM" :cols="runCols" />
        <RunRow v-for="r in runs" :key="r.id" :run="r" :agent-name="nameOf(r.agent)" :grid="RUN_GRID" :grid-sm="RUN_GRID_SM" />
      </div>
      <p v-else class="border-t border-line px-5 py-10 text-center text-[14px] text-ink3">No runs yet. They show here as routines fire.</p>
    </template>

    <!-- Workflows -->
    <template v-else-if="tab === 'workflows'">
      <LoadState v-if="flowsError || !flows" :loading="flowsLoading" :error="flowsError" label="workflows" class="px-5 pb-4" @retry="retryFlows" />
      <template v-else>
        <div v-if="flows.scripts.length" class="flex flex-wrap items-center gap-2 border-t border-line bg-panel2 px-5 py-3 text-[13px] text-ink2">
          <span class="mr-1 text-ink3">Scripts</span>
          <span v-for="s in flows.scripts" :key="s" class="inline-flex h-7 items-center gap-1.5 rounded-lg border border-line bg-panel px-2.5 font-mono text-[12px] text-ink">
            <FileCode2 class="size-3.5 text-ink3" aria-hidden="true" />{{ s }}
          </span>
        </div>
        <ul v-if="flows.runs.length" class="border-t border-line">
          <WorkflowRunRow v-for="r in flows.runs" :key="r.id" :run="r" />
        </ul>
        <div v-else class="flex flex-col items-center gap-3 border-t border-line px-5 py-14 text-center">
          <span class="grid size-11 place-items-center rounded-xl bg-panel2"><Workflow class="size-5 text-ink2" /></span>
          <p class="text-[15px] font-medium">{{ flows.scripts.length ? 'No workflow has run yet' : 'No workflow scripts yet' }}</p>
          <p class="max-w-md text-[13.5px] text-ink2">A workflow is a script in <code class="font-mono">workflows/</code> that puts agents to work in steps, waits, and asks you when it must.</p>
          <AskChip :icon="Workflow" words="Write a workflow that " label="Write a workflow" />
        </div>
      </template>
    </template>

    <!-- Hand-offs still out -->
    <template v-else>
      <ul v-if="out?.length" class="border-t border-line">
        <HandoffLine v-for="h in out" :key="h.id" :handoff="h" />
      </ul>
      <div v-else class="flex flex-col items-center gap-3 border-t border-line px-5 py-14 text-center">
        <span class="grid size-11 place-items-center rounded-xl bg-panel2"><MessagesSquare class="size-5 text-ink2" /></span>
        <p class="text-[15px] font-medium">Nothing is out between agents</p>
        <RouterLink to="/talk" class="text-[13.5px] text-ink2 underline decoration-line2 underline-offset-4 hover:text-ink">See all of Team talk</RouterLink>
      </div>
    </template>
  </section>
</template>

<style scoped>
.lf-select {
  background-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='%23888' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'%3E%3Cpath d='m6 9 6 6 6-6'/%3E%3C/svg%3E");
  background-size: 14px;
  background-position: right 8px center;
  background-repeat: no-repeat;
}
</style>
