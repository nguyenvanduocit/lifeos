<script setup lang="ts">
import EntityId from '../ds/EntityId.vue'
// One task, as the peek beside the list shows it and as its own page does (`page`): its title, its facts as a
// property list, its notes, the other open tasks of its goal, and what the Assistant can do with it.
import { computed } from 'vue'
import {
  CalendarClock, CalendarDays, CircleDot, ExternalLink, FileText, Flag, MessageSquare, Scissors, Tag, Target, X,
} from '@lucide/vue'
import { useRead } from '../../data'
import { longDay, todayIso } from '../../format'
import LoadState from '../LoadState.vue'
import Markdown from '../Markdown.vue'
import AskChip from '../AskChip.vue'
import PropertyList from '../ds/PropertyList.vue'
import StatusTag from '../ds/StatusTag.vue'
import TaskCheck from './TaskCheck.vue'
import Priority from './Priority.vue'
import { whenOf } from './when'

const props = defineProps<{ id: string; page?: boolean }>()
defineEmits<{ close: [] }>()

const { data, error, loading, retry } = useRead('read_doc', () => ({ id: `task_${props.id}` }))
const { data: goals } = useRead('list_goals', () => ({}))
const t = computed(() => data.value?.summary)
const done = computed(() => t.value?.status === 'done')
const goal = computed(() => goals.value?.find((g) => g.summary.id === t.value?.goal))
const { data: siblings } = useRead('list_tasks', () => ({ goal: t.value?.goal ?? '__none__' }))
const related = computed(() => (t.value?.goal ? (siblings.value ?? []).filter((x) => x.id !== props.id) : []))
const when = computed(() => (t.value ? whenOf(t.value) : null))

const rows = computed(() => {
  const s = t.value
  if (!s) return []
  return [
    { key: 'status', label: 'Status', icon: CircleDot, value: s.status ?? 'open' },
    { key: 'due', label: 'Due', icon: CalendarClock, value: s.due },
    { key: 'scheduled', label: 'Planned', icon: CalendarDays, value: s.scheduled },
    { key: 'priority', label: 'Priority', icon: Flag, value: s.priority ?? 'normal' },
    { key: 'goal', label: 'Goal', icon: Target, value: s.goal },
    { key: 'tags', label: 'Tags', icon: Tag, value: s.tags.length ? 'yes' : null },
    { key: 'path', label: 'File', icon: FileText, value: s.path },
  ]
})
const ref_ = computed(() => (t.value ? `task_${t.value.id}` : ''))
</script>

<template>
  <div :class="page ? '@container' : 'flex h-full flex-col'">
    <div v-if="!page" class="flex h-14 shrink-0 items-center gap-1 border-b border-line px-4">
      <span class="text-[13px] text-ink3">Task</span>
      <RouterLink :to="`/tasks/${id}`" class="ml-auto inline-flex h-8 items-center gap-1.5 rounded-lg px-2.5 text-[13px] text-ink2 hover:bg-panel2 hover:text-ink">
        <ExternalLink class="size-3.5" aria-hidden="true" /> Open page
      </RouterLink>
      <button type="button" class="grid size-8 place-items-center rounded-lg text-ink2 hover:bg-panel2 hover:text-ink" aria-label="Close" @click="$emit('close')">
        <X class="size-4" />
      </button>
    </div>

    <div :class="page ? '' : 'min-h-0 flex-1 overflow-y-auto px-6 py-5'">
      <LoadState :loading="loading && !data" :error="error" label="task" @retry="retry" />
      <template v-if="t">
        <div class="flex items-start gap-3">
          <TaskCheck :task="t" :done="done" class="mt-1.5" />
          <h2 class="min-w-0 flex-1 font-semibold tracking-[-0.02em] wrap-anywhere" :class="page ? 'text-[30px] leading-tight' : 'text-[22px] leading-snug'">{{ t.title }}</h2>
          <EntityId :key="t.id" :value="`task_${t.id}`" />
        </div>

        <div :class="page ? 'mt-8 grid items-start gap-8 @min-[52rem]:grid-cols-[minmax(0,1fr)_17rem]' : ''">
        <div :class="page ? 'order-2 rounded-2xl border border-line bg-panel2 px-4 py-4 @min-[52rem]:sticky @min-[52rem]:top-0' : 'mt-5 border-y border-line py-1.5'">
          <p v-if="page" class="mb-2 text-[12.5px] font-medium text-ink3">Details</p>
          <PropertyList :rows="rows" :stack="page">
            <template #status>
              <StatusTag :tone="done ? 'good' : when?.tone === 'critical' ? 'critical' : 'info'">{{ done ? 'Done' : when?.tone === 'critical' ? 'Late' : 'Open' }}</StatusTag>
            </template>
            <template #due>
              <span :class="t.due && t.due < todayIso() && !done ? 'font-medium text-critical' : ''">{{ t.due ? longDay(t.due) : '' }}</span>
              <span v-if="when && t.due" class="text-ink3">· {{ when.text }}</span>
            </template>
            <template #scheduled>{{ t.scheduled ? longDay(t.scheduled) : '' }}</template>
            <template #priority><Priority :priority="t.priority" /></template>
            <template #goal>
              <template v-if="goal">
                <RouterLink :to="`/goals/${encodeURIComponent(goal.summary.id)}`" class="truncate font-medium hover:underline">{{ goal.summary.title }}</RouterLink>
                <span class="h-1.5 w-16 shrink-0 overflow-hidden rounded-full bg-panel3"><span class="block h-full rounded-full bg-ink" :style="{ width: (goal.summary.progress ?? 0) + '%' }" /></span>
                <span class="text-[12.5px] text-ink3 tabular-nums">{{ goal.summary.progress ?? 0 }}%</span>
              </template>
              <span v-else class="text-ink3">{{ t.goal }}</span>
            </template>
            <template #tags>
              <span v-for="g in t.tags" :key="g" class="rounded-md bg-panel2 px-1.5 py-0.5 text-[12.5px] text-ink2">#{{ g }}</span>
            </template>
            <template #path><span class="truncate font-mono text-[12px] text-ink3">{{ t.path }}</span></template>
          </PropertyList>
        </div>

        <div :class="page ? 'order-1 min-w-0' : ''">
        <section :class="page ? '' : 'mt-6'">
          <h3 class="mb-2 text-[13px] font-medium text-ink3">Notes</h3>
          <Markdown v-if="data?.body.trim()" :source="data.body" class="text-[14.5px] leading-relaxed" />
          <div v-else class="rounded-xl border border-dashed border-line2 px-4 py-5 text-[14px] text-ink3" :class="page ? 'py-8 text-center' : ''">
            No notes yet.
            <AskChip class="ml-2" ghost :icon="FileText" :words="`Add notes to the task ${ref_}: `" label="Add notes" />
          </div>
        </section>

        <section v-if="related.length" class="mt-7">
          <h3 class="mb-2 flex items-center gap-2 text-[13px] font-medium text-ink3">Also for this goal <span class="tabular-nums">{{ related.length }}</span></h3>
          <ul class="overflow-hidden rounded-xl border border-line">
            <li v-for="r in related" :key="r.id" class="relative flex items-center gap-3 border-b border-line px-3.5 py-2.5 last:border-b-0 hover:bg-panel2/70">
              <span class="size-1.5 shrink-0 rounded-full bg-line2" aria-hidden="true" />
              <RouterLink :to="`/tasks/${r.id}`" class="min-w-0 flex-1 truncate text-[14px] after:absolute after:inset-0">{{ r.title }}</RouterLink>
              <span class="text-[12.5px] text-ink3">{{ whenOf(r)?.text ?? 'Anytime' }}</span>
            </li>
          </ul>
        </section>

        <section class="mt-7">
          <h3 class="mb-2.5 text-[13px] font-medium text-ink3">Ask the Assistant</h3>
          <div class="flex flex-wrap gap-2">
            <AskChip :icon="CalendarDays" :words="`Reschedule the task ${ref_} to `" label="Reschedule" />
            <AskChip :icon="Flag" :words="`Change the priority of the task ${ref_} to `" label="Priority" />
            <AskChip :icon="Target" :words="`Link the task ${ref_} to the goal `" label="Link to goal" />
            <AskChip :icon="Scissors" :words="`Split the task ${ref_} into smaller tasks.`" label="Split" />
            <AskChip :icon="MessageSquare" :words="`About the task ${ref_}: `" label="Ask about it" />
          </div>
        </section>
        </div>
        </div>
      </template>
    </div>
  </div>
</template>
