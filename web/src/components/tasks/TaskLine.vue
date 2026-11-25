<script setup lang="ts">
// A task as a table row (Relay's agents table): the check, the title over a grey line (goal, tags), when, priority.
// The row opens the task: in a peek beside the list when `peek` is set, else on its own page.
import { computed } from 'vue'
import { Target } from '@lucide/vue'
import type { DocSummary } from '../../core/generated/DocSummary'
import TaskCheck from './TaskCheck.vue'
import Priority from './Priority.vue'
import { whenOf } from './when'

const props = defineProps<{ task: DocSummary; goal?: string; showGoal?: boolean; done?: boolean; grid: string; gridSm?: string; to: string | object; selected?: boolean }>()
const when = computed(() => whenOf(props.task))
const sub = computed(() => props.task.tags.slice(0, 3).map((t) => '#' + t).join('  '))
</script>

<template>
  <div role="row" class="group/row relative grid min-h-[60px] items-center gap-x-4 border-b border-line px-5 py-2.5 transition-colors last:border-b-0 hover:bg-panel2/70"
    :class="selected ? 'bg-panel2' : ''" :style="{ '--cols': grid, '--cols-sm': gridSm ?? 'minmax(0,1fr) auto' }" data-cols>
    <span v-if="selected" class="absolute inset-y-0 left-0 w-0.5 bg-ink" aria-hidden="true" />
    <div role="cell" class="flex min-w-0 items-center gap-3.5">
      <TaskCheck :task="task" :done="done" />
      <RouterLink :to="to" class="min-w-0 after:absolute after:inset-0 focus-visible:outline-none" :aria-current="selected ? 'true' : undefined">
        <span class="block truncate text-[14.5px] font-medium" :class="done ? 'text-ink3 line-through decoration-ink3/60' : ''">{{ task.title }}</span>
        <span v-if="sub" class="block truncate text-[13px] text-ink3">{{ sub }}</span>
      </RouterLink>
    </div>
    <div v-if="showGoal !== false" role="cell" class="hidden min-w-0 items-center gap-2 text-[13.5px] text-ink2 @min-[44rem]:flex">
      <template v-if="goal"><Target class="size-3.5 shrink-0 text-ink3" aria-hidden="true" /><span class="truncate">{{ goal }}</span></template>
      <span v-else class="text-ink3">—</span>
    </div>
    <div role="cell" class="text-[13.5px] whitespace-nowrap tabular-nums"
      :class="when?.tone === 'critical' ? 'font-medium text-critical' : when?.tone === 'warning' ? 'text-ink' : 'text-ink2'">
      {{ done ? 'Done' : when?.text ?? 'Anytime' }}
    </div>
    <div role="cell" class="hidden @min-[44rem]:block"><Priority :priority="task.priority" /></div>
  </div>
</template>
