<script setup lang="ts">
import { Check, Flame } from '@lucide/vue'
import type { HabitStatus } from '../core/generated/HabitStatus'
import { ask, assistant } from '../assistant'
import Heatmap from './Heatmap.vue'

const props = defineProps<{ habit: HabitStatus; today: string; weeks?: number }>()

function done() {
  void ask(`Ghi thói quen “${props.habit.habit}” hôm nay.`)
}
</script>

<template>
  <li class="flex items-center gap-3 py-1.5">
    <button
      type="button"
      class="flex size-7 shrink-0 items-center justify-center rounded-full border"
      :class="habit.done_today ? 'border-good bg-good text-white' : 'border-line text-ink3 hover:border-accent hover:text-accent'"
      :disabled="habit.done_today || !!assistant.pending"
      :title="habit.done_today ? 'Hôm nay đã làm' : 'Nhờ agent ghi hôm nay đã làm'"
      @click="done"
    >
      <Check :size="15" />
    </button>
    <div class="min-w-0 flex-1">
      <div class="truncate text-sm font-medium first-letter:uppercase">{{ habit.habit }}</div>
      <div class="flex items-center gap-2 text-xs text-ink2">
        <span class="inline-flex items-center gap-0.5" :title="`Chuỗi ${habit.streak} ngày`"><Flame :size="12" />{{ habit.streak }} ngày</span>
        <span>{{ habit.done_this_week }}/{{ habit.target_per_week }} tuần này</span>
      </div>
    </div>
    <Heatmap :values="habit.recent.slice(-(weeks ?? 12) * 7)" :end="today" :unit="habit.unit" small />
  </li>
</template>
