<script setup lang="ts">
import { computed } from 'vue'
import type { BudgetLine } from '../core/generated/BudgetLine'
import Meter from './Meter.vue'
import { money } from '../format'

const props = defineProps<{ line: BudgetLine }>()
const ratio = computed(() => (props.line.budget > 0 ? props.line.spent / props.line.budget : 0))
const status = computed(() => (ratio.value > 1 ? 'critical' : ratio.value > 0.85 ? 'warning' : undefined))
</script>

<template>
  <li class="py-1.5">
    <div class="flex items-baseline gap-2 text-sm">
      <span class="flex-1 first-letter:uppercase">{{ line.category }}</span>
      <span class="tabular-nums text-xs text-ink2">{{ money(line.spent, line.currency) }} / {{ money(line.budget, line.currency) }}</span>
    </div>
    <Meter class="mt-1" :value="line.spent" :max="line.budget" :status="status" :label="`Ngân sách ${line.category}`" />
    <div v-if="status" class="mt-0.5 text-xs" :class="status === 'critical' ? 'text-critical' : 'text-ink2'">
      {{ status === 'critical' ? '▲ Vượt ngân sách' : '● Sắp chạm ngân sách' }} ({{ Math.round(ratio * 100) }}%)
    </div>
  </li>
</template>
