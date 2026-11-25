<script setup lang="ts">
// A goal as a table row: its mark and title over its "why", progress as a bar and a number, tasks open of all, the
// horizon, and its state. The whole row opens the goal's page.
import { computed } from 'vue'
import { Target } from '@lucide/vue'
import type { GoalView } from '../../core/generated/GoalView'
import IconTile from '../ds/IconTile.vue'
import StatusTag from '../ds/StatusTag.vue'
import { goalTone } from './goal'

const props = defineProps<{ goal: GoalView; grid: string; gridSm?: string }>()
const s = computed(() => props.goal.summary)
const total = computed(() => props.goal.open_tasks + props.goal.done_tasks)
const state = computed(() => goalTone(s.value.status))
</script>

<template>
  <div role="row" class="group/row relative grid min-h-[68px] items-center gap-x-4 border-b border-line px-5 py-3 transition-colors last:border-b-0 hover:bg-panel2/70"
    :style="{ '--cols': grid, '--cols-sm': gridSm ?? 'minmax(0,1fr) auto' }" data-cols>
    <div role="cell" class="flex min-w-0 items-center gap-3.5">
      <IconTile :icon="Target" :tone="s.status === 'active' ? 'goals' : 'neutral'" />
      <RouterLink :to="`/goals/${encodeURIComponent(s.id)}`" class="min-w-0 after:absolute after:inset-0 focus-visible:outline-none">
        <span class="block truncate text-[14.5px] font-medium" :class="s.status === 'dropped' ? 'text-ink3 line-through decoration-ink3/60' : ''">{{ s.title }}</span>
        <span v-if="goal.why" class="block truncate text-[13px] text-ink3">{{ goal.why }}</span>
      </RouterLink>
    </div>
    <div role="cell" class="flex min-w-0 items-center gap-2.5">
      <span class="hidden h-1.5 w-full max-w-28 overflow-hidden rounded-full bg-panel3 @min-[44rem]:block" aria-hidden="true">
        <span class="block h-full rounded-full" :class="s.status === 'done' ? 'bg-good' : 'bg-ink'" :style="{ width: (s.progress ?? 0) + '%' }" />
      </span>
      <span class="text-[13.5px] font-medium tabular-nums">{{ s.progress ?? 0 }}%</span>
    </div>
    <div role="cell" class="hidden text-[13.5px] text-ink2 tabular-nums @min-[44rem]:block">
      <template v-if="total"><span class="text-ink">{{ goal.open_tasks }}</span> open <span class="text-ink3">of {{ total }}</span></template>
      <span v-else class="text-ink3">No tasks</span>
    </div>
    <div role="cell" class="hidden text-[13.5px] text-ink2 @min-[44rem]:block">{{ s.horizon ?? s.due ?? '—' }}</div>
    <div role="cell" class="hidden @min-[44rem]:block"><StatusTag :tone="state.tone">{{ state.label }}</StatusTag></div>
  </div>
</template>
