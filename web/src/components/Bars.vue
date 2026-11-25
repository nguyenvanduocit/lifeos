<script setup lang="ts">
// One series of bars over days (e.g. spending per day), in the tone the chart sits in. Rounded data ends,
// a baseline, an optional dashed reference line (an average, a daily budget); hover a bar for its value.
// The highest bar is labelled directly.
import { computed, ref, watch } from 'vue'

const props = defineProps<{
  points: { label: string; value: number }[]
  format: (v: number) => string
  height?: number
  reference?: { value: number; label: string }
}>()

const H = computed(() => props.height ?? 120)
const PAD_TOP = 16
const W = 600
const max = computed(() => Math.max(0, props.reference?.value ?? 0, ...props.points.map((p) => p.value)))
const step = computed(() => W / Math.max(1, props.points.length))
const gap = computed(() => (step.value > 14 ? 3 : 2))
const barW = computed(() => Math.max(2, step.value - gap.value))
const peak = computed(() => {
  const top = Math.max(0, ...props.points.map((p) => p.value))
  return props.points.findIndex((p) => p.value === top && p.value > 0)
})
const y = (v: number) => H.value - (max.value > 0 ? ((H.value - PAD_TOP) * v) / max.value : 0)

function barPath(i: number, v: number): string {
  const h = H.value - y(v)
  if (h <= 0) return ''
  const x = i * step.value + gap.value / 2
  const top = H.value - h
  const r = Math.min(5, barW.value / 2, h)
  const w = barW.value
  return `M${x},${H.value} V${top + r} Q${x},${top} ${x + r},${top} H${x + w - r} Q${x + w},${top} ${x + w},${top + r} V${H.value} Z`
}

const hovered = ref<number | null>(null)
watch(() => props.points, () => { hovered.value = null })
const over = (v: number) => !!props.reference && v > props.reference.value
</script>

<template>
  <div>
    <div class="relative">
      <svg :viewBox="`0 0 ${W} ${H + 1}`" class="w-full" preserveAspectRatio="none" :style="{ height: H + 'px' }" role="img"
        :aria-label="`Daily bar chart, ${points.length} days${peak >= 0 ? ', peak ' + points[peak].label + ' ' + format(points[peak].value) : ''}`"
        @mouseleave="hovered = null">
        <rect v-for="(_, i) in points" :key="'hit' + i" :x="i * step" y="0" :width="step" :height="H" fill="transparent" @mouseenter="hovered = i" />
        <path
          v-for="(p, i) in points"
          :key="i"
          :d="barPath(i, p.value)"
          :fill="over(p.value) ? 'var(--critical)' : 'var(--tone)'"
          :opacity="hovered === null ? (i === peak ? 1 : 0.78) : hovered === i ? 1 : 0.35"
          class="transition-opacity"
          pointer-events="none"
        />
        <line v-if="reference" x1="0" :x2="W" :y1="y(reference.value)" :y2="y(reference.value)" stroke="var(--ink2)" stroke-width="1.25"
          stroke-dasharray="5 4" vector-effect="non-scaling-stroke" pointer-events="none" />
        <line x1="0" :x2="W" :y1="H" :y2="H" stroke="var(--line)" stroke-width="1" vector-effect="non-scaling-stroke" />
      </svg>
      <span v-if="reference && max > 0" class="pointer-events-none absolute right-0 -translate-y-full rounded bg-panel/85 px-1 text-[10px] font-semibold text-ink2"
        :style="{ top: (y(reference.value) / (H + 1)) * 100 + '%' }">{{ reference.label }} {{ format(reference.value) }}</span>
    </div>
    <div class="mt-1.5 flex h-4 justify-between gap-2 text-xs text-ink2">
      <span v-if="hovered !== null" class="truncate">{{ points[hovered].label }}: <b class="font-semibold text-ink">{{ format(points[hovered].value) }}</b></span>
      <span v-else-if="peak >= 0" class="truncate">Peak {{ points[peak].label }}: <b class="font-semibold text-ink">{{ format(points[peak].value) }}</b></span>
      <span v-else />
      <span v-if="points.length" class="shrink-0 text-ink3">{{ points[0].label }} – {{ points[points.length - 1].label }}</span>
    </div>
  </div>
</template>
