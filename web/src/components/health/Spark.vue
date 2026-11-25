<script setup lang="ts">
// A metric's recent days as thin bars, scaled between its low and high; the latest in ink.
import { computed } from 'vue'

const props = defineProps<{ points: { day: string; value: number }[]; count?: number; height?: string }>()
const shown = computed(() => props.points.slice(-(props.count ?? 24)))
const span = computed(() => {
  const v = shown.value.map((p) => p.value)
  return { lo: Math.min(...v), hi: Math.max(...v) }
})
const h = (v: number) => 18 + (span.value.hi === span.value.lo ? 42 : ((v - span.value.lo) / (span.value.hi - span.value.lo)) * 82)
</script>

<template>
  <div class="flex items-end gap-[2px]" :class="height ?? 'h-8'" aria-hidden="true">
    <span v-for="(p, i) in shown" :key="p.day" class="min-w-[2px] flex-1 rounded-[2px]"
      :class="i === shown.length - 1 ? 'bg-ink' : 'bg-line2'" :style="{ height: h(p.value) + '%' }" />
  </div>
</template>
