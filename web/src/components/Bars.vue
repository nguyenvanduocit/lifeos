<script setup lang="ts">
// One series of bars over days (e.g. spending per day). Thin bars with a 2px
// gap, rounded data end, a baseline; hover a bar for its value. The highest
// bar is labelled directly.
import { computed, ref } from 'vue'

const props = defineProps<{
  points: { label: string; value: number }[]
  format: (v: number) => string
  height?: number
}>()

const H = computed(() => props.height ?? 120)
const PAD_TOP = 16
const W = 600
const max = computed(() => Math.max(0, ...props.points.map((p) => p.value)))
const step = computed(() => W / Math.max(1, props.points.length))
const barW = computed(() => Math.max(2, step.value - 2))
const peak = computed(() => props.points.findIndex((p) => p.value === max.value && p.value > 0))

function barPath(i: number, v: number): string {
  const h = max.value > 0 ? ((H.value - PAD_TOP) * v) / max.value : 0
  if (h <= 0) return ''
  const x = i * step.value + 1
  const y = H.value - h
  const r = Math.min(4, barW.value / 2, h)
  const w = barW.value
  return `M${x},${H.value} V${y + r} Q${x},${y} ${x + r},${y} H${x + w - r} Q${x + w},${y} ${x + w},${y + r} V${H.value} Z`
}

const hovered = ref<number | null>(null)
</script>

<template>
  <div>
    <svg :viewBox="`0 0 ${W} ${H + 1}`" class="w-full" preserveAspectRatio="none" :style="{ height: H + 'px' }" role="img" aria-label="Biểu đồ cột theo ngày" @mouseleave="hovered = null">
      <rect
        v-for="(_, i) in points"
        :key="'hit' + i"
        :x="i * step"
        y="0"
        :width="step"
        :height="H"
        fill="transparent"
        @mouseenter="hovered = i"
      />
      <path
        v-for="(p, i) in points"
        :key="i"
        :d="barPath(i, p.value)"
        :fill="'var(--series)'"
        :opacity="hovered === null || hovered === i ? 1 : 0.45"
        pointer-events="none"
      />
      <line x1="0" :x2="W" :y1="H" :y2="H" stroke="var(--line)" stroke-width="1" vector-effect="non-scaling-stroke" />
    </svg>
    <div class="mt-1 flex h-4 justify-between text-xs text-ink2">
      <span v-if="hovered !== null">{{ points[hovered].label }}: <b class="font-medium text-ink">{{ format(points[hovered].value) }}</b></span>
      <span v-else-if="peak >= 0">Cao nhất {{ points[peak].label }}: <b class="font-medium text-ink">{{ format(max) }}</b></span>
      <span v-if="points.length" class="text-ink3">{{ points[0].label }} – {{ points[points.length - 1].label }}</span>
    </div>
  </div>
</template>
