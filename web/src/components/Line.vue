<script setup lang="ts">
// A measurement over days: a line in the tone it sits in over a soft area, a crosshair and the nearest value
// on hover. Days without a value are skipped, not zero.
import { computed, ref, useId, watch } from 'vue'
import { num, parseDay, shortDay } from '../format'

const props = defineProps<{ points: { day: string; value: number }[]; unit?: string | null; height?: number; spark?: boolean }>()

const W = 600
const H = computed(() => props.height ?? (props.spark ? 40 : 140))
const PAD = 8
const gradient = 'line-area-' + useId()

const values = computed(() => props.points.map((p) => p.value))
const xs = computed(() => props.points.map((p) => parseDay(p.day).getTime()))
const span = computed(() => {
  const lo = Math.min(...values.value), hi = Math.max(...values.value)
  const pad = (hi - lo) * 0.15 || Math.abs(hi) * 0.05 || 1
  return { lo: lo - pad, hi: hi + pad }
})
const ticks = computed(() => {
  const lo = Math.min(...values.value)
  const hi = Math.max(...values.value)
  const midpoint = lo + (hi - lo) / 2
  return [...new Map([hi, midpoint, lo].map((value) => [num(value), value]))]
    .map(([label, value]) => ({ label, value }))
})

const t0 = computed(() => Math.min(...xs.value))
const t1 = computed(() => Math.max(...xs.value))
function x(i: number): number {
  return t1.value === t0.value ? W / 2 : PAD + ((xs.value[i] - t0.value) / (t1.value - t0.value)) * (W - 2 * PAD)
}
function y(v: number): number {
  return PAD + (1 - (v - span.value.lo) / (span.value.hi - span.value.lo)) * (H.value - 2 * PAD)
}
const path = computed(() => props.points.map((p, i) => `${i ? 'L' : 'M'}${x(i)},${y(p.value)}`).join(' '))
const area = computed(() =>
  props.points.length > 1 ? `${path.value} L${x(props.points.length - 1)},${H.value} L${x(0)},${H.value} Z` : '',
)

const hovered = ref<number | null>(null)
watch(() => props.points, () => { hovered.value = null })
const svg = ref<SVGSVGElement>()
function move(e: MouseEvent) {
  const box = svg.value?.getBoundingClientRect()
  if (!box || !props.points.length) return
  const px = ((e.clientX - box.left) / box.width) * W
  let best = 0
  props.points.forEach((_, i) => {
    if (Math.abs(x(i) - px) < Math.abs(x(best) - px)) best = i
  })
  hovered.value = best
}
const unit = computed(() => (props.unit ? ' ' + props.unit : ''))
</script>

<template>
  <div v-if="points.length">
    <div class="flex gap-2">
      <div v-if="!spark" class="relative min-w-9 shrink-0 text-right text-xs tabular-nums text-ink3" aria-hidden="true">
        <div v-for="tick in ticks" :key="tick.label" class="grid">
          <span class="invisible h-0">{{ tick.label }}</span>
          <span class="absolute right-0 -translate-y-1/2" :style="{ top: y(tick.value) + 'px' }">{{ tick.label }}</span>
        </div>
      </div>
      <div class="relative min-w-0 flex-1" :style="{ height: H + 'px' }">
        <svg
          ref="svg"
          :viewBox="`0 0 ${W} ${H}`"
          preserveAspectRatio="none"
          class="absolute inset-0 h-full w-full"
          role="img"
          :aria-label="`${points.length} measurements, latest ${num(points[points.length - 1].value)}${unit}`"
          @mousemove="move"
          @mouseleave="hovered = null"
        >
          <defs>
            <linearGradient :id="gradient" x1="0" x2="0" y1="0" y2="1">
              <stop offset="0" stop-color="var(--tone)" :stop-opacity="spark ? 0.28 : 0.32" />
              <stop offset="1" stop-color="var(--tone)" stop-opacity="0" />
            </linearGradient>
          </defs>
          <line v-for="tick in spark ? [] : ticks" :key="tick.label" x1="0" :x2="W" :y1="y(tick.value)" :y2="y(tick.value)" stroke="var(--line)" stroke-dasharray="2 4" vector-effect="non-scaling-stroke" />
          <path v-if="area" :d="area" :fill="`url(#${gradient})`" />
          <line v-if="hovered !== null" :x1="x(hovered)" :x2="x(hovered)" y1="0" :y2="H" stroke="var(--ink3)" stroke-dasharray="3 3" vector-effect="non-scaling-stroke" />
          <path :d="path" fill="none" stroke="var(--tone)" :stroke-width="spark ? 2 : 2.5" stroke-linejoin="round" stroke-linecap="round" vector-effect="non-scaling-stroke" />
        </svg>
        <!-- markers in HTML so they stay round under the stretched viewBox -->
        <span
          v-for="(p, i) in points"
          v-show="i === points.length - 1 || i === hovered"
          :key="p.day"
          class="pointer-events-none absolute -translate-x-1/2 -translate-y-1/2 rounded-full border-2"
          :class="i === hovered ? 'size-3' : spark ? 'size-2' : 'size-2.5'"
          :style="{ left: (x(i) / W) * 100 + '%', top: y(p.value) + 'px', background: 'var(--tone)', borderColor: 'var(--panel)' }"
        />
      </div>
    </div>
    <div v-if="!spark" class="mt-1.5 flex h-4 justify-between text-xs text-ink2">
      <span v-if="hovered !== null">{{ shortDay(points[hovered].day) }}: <b class="font-semibold text-ink">{{ num(points[hovered].value) }}{{ unit }}</b></span>
      <span v-else />
      <span class="text-ink3">{{ shortDay(points[0].day) }} – {{ shortDay(points[points.length - 1].day) }}</span>
    </div>
  </div>
</template>
