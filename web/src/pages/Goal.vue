<script setup lang="ts">
import EntityId from '../components/ds/EntityId.vue'
// A goal on its own page, laid out like a task's (docs/design.md, "Detail"): the trail back to Goals, the title and
// progress, its tasks (open, then done) as table rows, its notes, and at the right a Details card with its facts.
import { computed } from 'vue'
import {
  CalendarClock, CalendarDays, CalendarRange, ChevronRight, CircleDot, ClipboardCheck, FileText, Gauge, ListChecks,
  ListPlus, MessageSquare, Pause, Plus, Tag, Target,
} from '@lucide/vue'
import { useRead } from '../data'
import { longDay } from '../format'
import AskChip from '../components/AskChip.vue'
import LoadState from '../components/LoadState.vue'
import Markdown from '../components/Markdown.vue'
import IconTile from '../components/ds/IconTile.vue'
import Panel from '../components/ds/Panel.vue'
import PropertyList from '../components/ds/PropertyList.vue'
import StatusTag from '../components/ds/StatusTag.vue'
import TableHead from '../components/ds/TableHead.vue'
import TaskLine from '../components/tasks/TaskLine.vue'
import { goalTone } from '../components/goals/goal'

const props = defineProps<{ id: string }>()
const { data, error, loading, retry } = useRead('read_doc', () => ({ id: `goal_${props.id}` }))
const { data: goals } = useRead('list_goals', () => ({}))
const { data: tasks } = useRead('list_tasks', () => ({ goal: props.id, status: 'all' as const }))

const g = computed(() => data.value?.summary)
const view = computed(() => goals.value?.find((x) => x.summary.id === props.id))
const open = computed(() => (tasks.value ?? []).filter((t) => t.status !== 'done'))
const done = computed(() => (tasks.value ?? []).filter((t) => t.status === 'done'))
const state = computed(() => goalTone(g.value?.status ?? null))
const progress = computed(() => g.value?.progress ?? 0)

const rows = computed(() => {
  const s = g.value
  if (!s) return []
  return [
    { key: 'status', label: 'Status', icon: CircleDot, value: s.status ?? 'active' },
    { key: 'progress', label: 'Progress', icon: Gauge, value: String(progress.value) },
    { key: 'horizon', label: 'Horizon', icon: CalendarRange, value: s.horizon },
    { key: 'due', label: 'Target date', icon: CalendarClock, value: s.due },
    { key: 'tasks', label: 'Tasks', icon: ListChecks, value: tasks.value ? String(tasks.value.length) : null },
    { key: 'tags', label: 'Tags', icon: Tag, value: s.tags.length ? 'yes' : null },
    { key: 'path', label: 'File', icon: FileText, value: s.path },
  ]
})
const GRID = 'minmax(0,3fr) minmax(0,1fr) minmax(0,0.6fr)'
const cols = [
  { label: 'Task', icon: ListChecks },
  { label: 'When', icon: CalendarDays },
  { label: 'Priority', icon: Target, class: 'hidden @min-[44rem]:flex' },
]
// The body without what the page already shows: the "why" section (it sits under the title) and empty sections.
const notes = computed(() => {
  const body = data.value?.body ?? ''
  const why = view.value?.why.trim() ?? ''
  const parts = body.split(/^(?=#{1,3} )/m)
  return parts.filter((p) => {
    const m = p.match(/^#{1,3} .*\n?([\s\S]*)$/)
    if (!m) return p.trim() !== ''
    const rest = m[1].trim()
    return rest !== '' && rest !== why
  }).join('').trim()
})
const ref_ = computed(() => (g.value ? `goal_${g.value.id}` : ''))
</script>

<template>
  <nav aria-label="Breadcrumb" class="mb-6 flex items-center gap-1.5 text-[14px] text-ink2">
    <RouterLink to="/goals" class="hover:text-ink">Goals</RouterLink>
    <ChevronRight class="size-3.5 text-ink3" aria-hidden="true" />
    <span class="truncate text-ink" aria-current="page">{{ g?.title ?? '…' }}</span>
  </nav>

  <LoadState :loading="loading && !data" :error="error" label="goal" class="mb-4" @retry="retry" />

  <div v-if="g" class="@container max-w-[1080px]">
    <header class="flex items-start gap-4">
      <IconTile :icon="Target" :tone="g.status === 'active' ? 'goals' : 'neutral'" size="lg" />
      <div class="min-w-0 flex-1">
        <h1 class="text-[30px] leading-tight font-semibold tracking-[-0.02em] wrap-anywhere">{{ g.title }}</h1>
        <EntityId :key="g.id" :value="`goal_${g.id}`" />
        <p v-if="view?.why" class="mt-1 text-[15px] text-ink2">{{ view.why }}</p>
      </div>
    </header>

    <div class="mt-6 flex flex-wrap items-center gap-x-5 gap-y-2 rounded-2xl border border-line bg-panel2 px-5 py-4">
      <span class="text-[28px] leading-none font-semibold tracking-tight tabular-nums">{{ progress }}%</span>
      <span class="h-2 min-w-40 flex-1 overflow-hidden rounded-full bg-panel3" role="progressbar" :aria-valuenow="progress" aria-valuemin="0" aria-valuemax="100" aria-label="Progress">
        <span class="block h-full rounded-full" :class="g.status === 'done' ? 'bg-good' : 'bg-ink'" :style="{ width: progress + '%' }" />
      </span>
      <span class="text-[13.5px] text-ink2 tabular-nums">
        <template v-if="tasks?.length">{{ done.length }} of {{ tasks.length }} tasks done</template>
        <template v-else>No tasks yet</template>
      </span>
    </div>

    <div class="mt-6 grid items-start gap-6 @min-[52rem]:grid-cols-[minmax(0,1fr)_17rem]">
      <div class="order-2 rounded-2xl border border-line bg-panel2 px-4 py-4 @min-[52rem]:sticky @min-[52rem]:top-0">
        <p class="mb-2 text-[12.5px] font-medium text-ink3">Details</p>
        <PropertyList :rows="rows" stack>
          <template #status><StatusTag :tone="state.tone">{{ state.label }}</StatusTag></template>
          <template #progress><span class="tabular-nums">{{ progress }}%</span></template>
          <template #due>{{ g.due ? longDay(g.due) : '' }}</template>
          <template #tasks><span class="tabular-nums">{{ open.length }} open · {{ done.length }} done</span></template>
          <template #tags>
            <span v-for="t in g.tags" :key="t" class="rounded-md bg-panel px-1.5 py-0.5 text-[12.5px] text-ink2">#{{ t }}</span>
          </template>
          <template #path><span class="truncate font-mono text-[12px] text-ink3">{{ g.path }}</span></template>
        </PropertyList>
      </div>

      <div class="order-1 min-w-0 space-y-6">
        <Panel title="Tasks" :icon="ListChecks" :count="open.length ? `${open.length} open` : undefined">
          <template #actions>
            <AskChip :icon="Plus" :words="`Add a task for the goal ${ref_}: `" label="Add task" />
            <AskChip :icon="ListPlus" :words="`Suggest the next tasks for the goal ${ref_}.`" label="Next steps" />
          </template>
          <div v-if="tasks?.length" role="table" aria-label="Tasks for this goal">
            <TableHead :grid="GRID" :cols="cols" />
            <TaskLine v-for="t in open" :key="t.id" :task="t" :show-goal="false" :grid="GRID" :to="`/tasks/${encodeURIComponent(t.id)}`" />
            <template v-if="done.length">
              <div class="flex h-9 items-center gap-2 border-y border-line bg-panel px-5 text-[13px] font-medium text-ink2">
                Done <span class="tabular-nums text-ink3">{{ done.length }}</span>
              </div>
              <TaskLine v-for="t in done" :key="t.id" :task="t" done :show-goal="false" :grid="GRID" :to="`/tasks/${encodeURIComponent(t.id)}`" />
            </template>
          </div>
          <p v-else class="border-t border-line px-5 py-8 text-center text-[14px] text-ink3">
            No tasks toward this goal yet. Ask for the next steps to get going.
          </p>
        </Panel>

        <section>
          <h3 class="mb-2 text-[13px] font-medium text-ink3">Notes</h3>
          <Markdown v-if="notes" :source="notes" class="max-w-[72ch] text-[14.5px] leading-relaxed wrap-anywhere" />
          <div v-else class="rounded-xl border border-dashed border-line2 px-4 py-8 text-center text-[14px] text-ink3">
            No notes yet.
            <AskChip class="ml-2" ghost :icon="FileText" :words="`Add notes to the goal ${ref_}: `" label="Add notes" />
          </div>
        </section>

        <section>
          <h3 class="mb-2.5 text-[13px] font-medium text-ink3">Ask the Assistant</h3>
          <div class="flex flex-wrap gap-2">
            <AskChip :icon="Gauge" :words="`Update the progress of the goal ${ref_} to `" label="Update progress" />
            <AskChip :icon="ClipboardCheck" :words="`Review the goal ${ref_}: where it stands and what is next.`" label="Review it" />
            <AskChip v-if="g.status === 'active'" :icon="Pause" :words="`Pause the goal ${ref_} because `" label="Pause" />
            <AskChip :icon="MessageSquare" :words="`About the goal ${ref_}: `" label="Ask about it" />
          </div>
        </section>
      </div>
    </div>
  </div>
</template>
