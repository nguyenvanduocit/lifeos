<script setup lang="ts">
import type { GoalView } from '../core/generated/GoalView'
import Meter from './Meter.vue'
import { relativeDay } from '../format'

defineProps<{ goal: GoalView; full?: boolean }>()
</script>

<template>
  <li class="py-2">
    <div class="flex items-baseline gap-2">
      <RouterLink :to="`/doc/${goal.summary.id}`" class="min-w-0 flex-1 truncate text-sm font-medium hover:underline">{{ goal.summary.title }}</RouterLink>
      <span class="text-xs tabular-nums text-ink2">{{ goal.summary.progress ?? 0 }}%</span>
    </div>
    <Meter class="mt-1.5" :value="goal.summary.progress ?? 0" :max="100" :label="`Tiến độ ${goal.summary.title}`" />
    <div class="mt-1 flex gap-3 text-xs text-ink3">
      <span v-if="goal.summary.horizon">{{ goal.summary.horizon }}</span>
      <span v-if="goal.summary.due">hạn {{ relativeDay(goal.summary.due) }}</span>
      <span v-if="goal.open_tasks || goal.done_tasks">{{ goal.done_tasks }}/{{ goal.open_tasks + goal.done_tasks }} việc xong</span>
      <span v-if="goal.summary.status && goal.summary.status !== 'active'" class="uppercase">{{ goal.summary.status }}</span>
    </div>
    <p v-if="full && goal.why" class="mt-1.5 text-sm text-ink2">{{ goal.why }}</p>
  </li>
</template>
