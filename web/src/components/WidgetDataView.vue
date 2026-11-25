<script setup lang="ts">
import { computed } from 'vue'
import type { WidgetData } from '../core/generated/WidgetData'
import BlockChart from './assistant/BlockChart.vue'

const props = defineProps<{ content: WidgetData; title: string; chartId: string }>()
const chart = computed(() => {
  const data = props.content
  if (data?.kind !== 'bars' && data?.kind !== 'line') return
  return {
    id: props.chartId,
    label: props.title,
    kind: data.kind === 'bars' ? 'bar' as const : 'line' as const,
    x: data.labels,
    unit: data.unit,
    series: [{ name: props.title, values: data.values }],
  }
})
const format = new Intl.NumberFormat(undefined, { maximumFractionDigits: 3 })
</script>

<template>
  <div v-if="content.kind === 'kpi'">
    <p class="break-words text-3xl font-semibold tabular-nums">
      {{ format.format(content.value) }}
      <span v-if="content.unit" class="text-sm font-medium text-ink2">{{ content.unit }}</span>
    </p>
    <p v-if="content.detail" class="mt-2 break-words text-sm text-ink2">{{ content.detail }}</p>
  </div>
  <div v-else-if="content.kind === 'goal'" class="space-y-2 text-sm">
    <div class="flex justify-between gap-2">
      <span class="break-words">{{ content.label }}</span>
      <span class="font-medium tabular-nums">{{ format.format(content.percent) }}%</span>
    </div>
    <div class="h-2 overflow-hidden rounded-full bg-panel3" role="progressbar" :aria-label="content.label"
      :aria-valuenow="content.percent" aria-valuemin="0" aria-valuemax="100">
      <div class="h-full rounded-full bg-good" :style="{ width: content.percent + '%' }" />
    </div>
  </div>
  <BlockChart v-else-if="chart" :part="chart" />
  <ul v-else-if="content.kind === 'list'" class="divide-y divide-line text-sm">
    <li v-for="(row, index) in content.rows" :key="index" class="break-words py-2 first:pt-0 last:pb-0">
      <p class="font-medium">{{ row.label }}</p>
      <p v-if="row.detail" class="mt-0.5 text-ink2">{{ row.detail }}</p>
    </li>
  </ul>
  <p v-else-if="content.kind === 'text'" class="whitespace-pre-wrap break-words text-sm leading-relaxed">{{ content.text }}</p>
</template>
