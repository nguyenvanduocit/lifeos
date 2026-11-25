<script setup lang="ts">
// A part-of-a-whole bar. Over the whole, the fill turns to a status colour,
// and the caller shows a label beside it, so colour never speaks alone.
import { computed } from 'vue'

const props = defineProps<{ value: number; max: number; status?: 'good' | 'warning' | 'critical'; label?: string }>()
const pct = computed(() => (props.max > 0 ? Math.min(100, Math.max(0, (props.value / props.max) * 100)) : 0))
const color = computed(() =>
  props.status === 'critical' ? 'var(--critical)' : props.status === 'warning' ? 'var(--warning)' : props.status === 'good' ? 'var(--good)' : 'var(--series)',
)
</script>

<template>
  <div
    class="h-2 w-full overflow-hidden rounded bg-panel2"
    role="meter"
    :aria-valuenow="value"
    :aria-valuemin="0"
    :aria-valuemax="max"
    :aria-label="label"
  >
    <div class="h-full rounded" :style="{ width: pct + '%', background: color }" />
  </div>
</template>
