<script setup lang="ts">
// A habit's last weeks, one cell a day (columns are weeks, Monday on top).
// Sequential blue: darker is more. Hover a cell for its day and value.
import { computed, ref } from 'vue'
import { isoDay, num, parseDay, shortDay } from '../format'

const props = defineProps<{ values: number[]; end: string; unit?: string | null; small?: boolean }>()

const CELL = computed(() => (props.small ? 9 : 12))
const GAP = 2

interface Cell { day: string; value: number; col: number; row: number }

const cells = computed((): Cell[] => {
  const end = parseDay(props.end)
  const n = props.values.length
  // Monday-based weekday of the end day: 0..6
  const endRow = (end.getDay() + 6) % 7
  return props.values.map((value, i) => {
    const back = n - 1 - i
    const d = new Date(end)
    d.setDate(end.getDate() - back)
    const slot = back - endRow // days back from the Monday of the end week
    const col = slot <= 0 ? 0 : Math.ceil(slot / 7)
    const row = (d.getDay() + 6) % 7
    return { day: isoDay(d), value, col, row }
  })
})

const columns = computed(() => Math.max(1, ...cells.value.map((c) => c.col + 1)))
const width = computed(() => columns.value * (CELL.value + GAP))
const height = computed(() => 7 * (CELL.value + GAP))

const max = computed(() => Math.max(...props.values, 0))
function fill(v: number): string {
  if (v <= 0) return 'var(--heat-0)'
  if (max.value <= 1) return 'var(--heat-3)'
  const t = v / max.value
  return t > 0.75 ? 'var(--heat-4)' : t > 0.5 ? 'var(--heat-3)' : t > 0.25 ? 'var(--heat-2)' : 'var(--heat-1)'
}

const hovered = ref<Cell | null>(null)
const caption = computed(() => {
  const c = hovered.value
  if (!c) return ''
  if (c.value <= 0) return `${shortDay(c.day)}: chưa làm`
  return `${shortDay(c.day)}: ${max.value <= 1 ? 'đã làm' : num(c.value) + (props.unit ? ' ' + props.unit : '')}`
})
</script>

<template>
  <div class="relative inline-block">
    <svg :width="width" :height="height" role="img" :aria-label="`${values.filter((v) => v > 0).length} ngày đã làm trong ${values.length} ngày`" @mouseleave="hovered = null">
      <rect
        v-for="c in cells"
        :key="c.day"
        :x="(columns - 1 - c.col) * (CELL + GAP)"
        :y="c.row * (CELL + GAP)"
        :width="CELL"
        :height="CELL"
        rx="2"
        :fill="fill(c.value)"
        :stroke="hovered?.day === c.day ? 'var(--ink)' : 'none'"
        stroke-width="1.5"
        @mouseenter="hovered = c"
      />
    </svg>
    <div v-if="!small" class="h-4 text-xs text-ink2">{{ caption }}</div>
    <div v-else-if="hovered" class="absolute left-0 top-full z-10 whitespace-nowrap rounded bg-ink px-1.5 py-0.5 text-xs text-panel">{{ caption }}</div>
  </div>
</template>
