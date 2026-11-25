<script setup lang="ts">
// A habit's last weeks as a grid that fills its width: a column a week (Monday on top), ink for done days, shaded by
// how much when the habit counts something; month names over the weeks they start, hover for a day.
import { computed, ref } from 'vue'
import { isoDay, num, parseDay, shortDay } from '../../format'

const props = defineProps<{ values: number[]; end: string; unit?: string | null }>()
const max = computed(() => Math.max(0, ...props.values))
const weeks = computed(() => {
  const end = parseDay(props.end)
  const days = props.values.map((value, i) => {
    const d = new Date(end)
    d.setDate(end.getDate() - (props.values.length - 1 - i))
    return { day: isoDay(d), value, row: (d.getDay() + 6) % 7 }
  })
  const out: ({ day: string; value: number } | null)[][] = []
  let col: ({ day: string; value: number } | null)[] = Array(days[0]?.row ?? 0).fill(null)
  for (const d of days) {
    col.push({ day: d.day, value: d.value })
    if (d.row === 6) { out.push(col); col = [] }
  }
  if (col.length) out.push([...col, ...Array(7 - col.length).fill(null)])
  return out
})
const monthFmt = new Intl.DateTimeFormat('en-US', { month: 'short' })
const months = computed(() => weeks.value.map((w, i) => {
  const first = w.find((c) => c)?.day
  if (!first) return ''
  const prev = weeks.value[i - 1]?.find((c) => c)?.day
  return !prev || prev.slice(0, 7) !== first.slice(0, 7) ? monthFmt.format(parseDay(first)) : ''
}))
const fill = (v: number) => {
  if (v <= 0) return undefined
  const t = max.value <= 1 ? 1 : 0.4 + 0.6 * (v / max.value)
  return { background: `color-mix(in oklab, var(--ink) ${Math.round(t * 85)}%, transparent)` }
}
const hovered = ref<{ day: string; value: number } | null>(null)
const caption = computed(() => {
  const c = hovered.value
  if (!c) return `${props.values.filter((v) => v > 0).length} of ${props.values.length} days done`
  if (c.value <= 0) return `${shortDay(c.day)}: not done`
  return `${shortDay(c.day)}: ${max.value <= 1 ? 'done' : num(c.value) + (props.unit ? ' ' + props.unit : '')}`
})
</script>

<template>
  <div>
    <div class="grid gap-[5px]" :style="{ gridTemplateColumns: `1.5rem repeat(${weeks.length}, minmax(0, 1fr))`, maxWidth: `${24 + weeks.length * 40}px` }" @mouseleave="hovered = null">
      <span />
      <span v-for="(m, i) in months" :key="'m' + i" class="h-4 overflow-visible text-[11.5px] whitespace-nowrap text-ink3">{{ m }}</span>
      <template v-for="row in 7" :key="row">
        <span class="text-[11px] leading-none text-ink3 self-center">{{ ['Mo', '', 'We', '', 'Fr', '', 'Su'][row - 1] }}</span>
        <span v-for="(w, i) in weeks" :key="i + '-' + row"
          class="aspect-square rounded-[5px]"
          :class="!w[row - 1] ? '' : w[row - 1]!.value > 0 ? '' : 'bg-panel3'"
          :style="w[row - 1] ? fill(w[row - 1]!.value) : undefined"
          :aria-hidden="true"
          @mouseenter="hovered = w[row - 1]" />
      </template>
    </div>
    <p class="mt-3 text-[13px] text-ink2" aria-live="polite">{{ caption }}</p>
  </div>
</template>
