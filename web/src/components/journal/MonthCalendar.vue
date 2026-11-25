<script setup lang="ts">
// A month of journal days: a day with an entry is shaded by how much was written and opens that day's page; today
// is ringed, the open day (`current`) is ink. The month shown follows `current`, and the arrows move it.
import { computed, ref, watch } from 'vue'
import { ChevronLeft, ChevronRight } from '@lucide/vue'
import { isoDay, longDay, parseDay, todayIso } from '../../format'

const props = defineProps<{ words: Map<string, number>; current?: string }>()
const today = todayIso()
const month = ref((props.current ?? today).slice(0, 7))
watch(() => props.current, (d) => d && (month.value = d.slice(0, 7)))
const max = computed(() => Math.max(1, ...props.words.values()))
const label = computed(() => new Intl.DateTimeFormat('en-US', { month: 'long', year: 'numeric' }).format(parseDay(month.value + '-01')))

function shift(by: number) {
  const d = parseDay(month.value + '-01')
  d.setMonth(d.getMonth() + by)
  month.value = isoDay(d).slice(0, 7)
}
const cells = computed(() => {
  const first = parseDay(month.value + '-01')
  const lead = (first.getDay() + 6) % 7
  const n = new Date(first.getFullYear(), first.getMonth() + 1, 0).getDate()
  const out: ({ day: string; n: number; words?: number } | null)[] = Array(lead).fill(null)
  for (let i = 1; i <= n; i++) {
    const day = `${month.value}-${String(i).padStart(2, '0')}`
    out.push({ day, n: i, words: props.words.get(day) })
  }
  return out
})
const shade = (w?: number) => (w ? { background: `color-mix(in oklab, var(--ink) ${Math.round(10 + (w / max.value) * 22)}%, transparent)` } : undefined)
</script>

<template>
  <section class="rounded-2xl border border-line bg-panel p-4" aria-label="Calendar">
    <div class="mb-3 flex items-center gap-1">
      <h2 class="flex-1 text-[15px] font-semibold">{{ label }}</h2>
      <button type="button" class="grid size-8 place-items-center rounded-lg text-ink2 hover:bg-panel2 hover:text-ink" aria-label="Previous month" @click="shift(-1)">
        <ChevronLeft class="size-4" />
      </button>
      <button type="button" class="grid size-8 place-items-center rounded-lg text-ink2 hover:bg-panel2 hover:text-ink disabled:opacity-30"
        :disabled="month >= today.slice(0, 7)" aria-label="Next month" @click="shift(1)">
        <ChevronRight class="size-4" />
      </button>
    </div>
    <div class="grid grid-cols-7 gap-1 text-center text-[11px] text-ink3" aria-hidden="true">
      <span v-for="(w, i) in ['Mo', 'Tu', 'We', 'Th', 'Fr', 'Sa', 'Su']" :key="i">{{ w }}</span>
    </div>
    <div class="mt-1 grid grid-cols-7 gap-1">
      <template v-for="(c, i) in cells" :key="c?.day ?? 'pad' + i">
        <span v-if="!c" aria-hidden="true" />
        <span v-else-if="c.day > today" class="grid aspect-square place-items-center text-[13px] text-ink3/50 tabular-nums">{{ c.n }}</span>
        <RouterLink v-else :to="`/journal/${c.day}`"
          class="grid aspect-square place-items-center rounded-lg text-[13px] tabular-nums transition-colors focus-visible:outline-2 focus-visible:outline-ring"
          :class="[
            current === c.day ? 'bg-ink! font-semibold text-panel' : c.words ? 'font-medium text-ink hover:ring-1 hover:ring-line2' : 'text-ink2 hover:bg-panel2',
            c.day === today && current !== c.day ? 'ring-1 ring-ink3' : '',
          ]"
          :style="current === c.day ? undefined : shade(c.words)"
          :aria-current="current === c.day ? 'page' : undefined"
          :aria-label="longDay(c.day) + (c.words ? `, ${c.words} words` : ', no entry')">
          {{ c.n }}
        </RouterLink>
      </template>
    </div>
    <p class="mt-3 flex items-center gap-1.5 text-[12px] text-ink3">
      Fewer words
      <span v-for="p in [10, 17, 24, 32]" :key="p" class="size-2.5 rounded-sm" :style="{ background: `color-mix(in oklab, var(--ink) ${p}%, transparent)` }" />
      More
    </p>
  </section>
</template>
