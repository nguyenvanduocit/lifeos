<script setup lang="ts">
// A letter on a colour: the Assistant is violet; each crew member keeps one
// colour (by its id, never by its place in a list).
import { computed } from 'vue'
import { ASSISTANT } from '../../assistant'

const props = defineProps<{ id: string; name: string; size?: number }>()
const COLORS = ['#2a78d6', '#eb6834', '#1baf7a', '#eda100', '#e87ba4', '#008300', '#e34948']
const bg = computed(() => {
  if (props.id === ASSISTANT) return 'var(--violet)'
  let h = 0
  for (const c of props.id) h = (h * 31 + c.charCodeAt(0)) >>> 0
  return COLORS[h % COLORS.length]
})
const letter = computed(() => (props.name.trim()[0] ?? '?').toUpperCase())
</script>

<template>
  <span
    class="inline-flex shrink-0 items-center justify-center rounded-md font-semibold text-white"
    :style="{ background: bg, width: (size ?? 22) + 'px', height: (size ?? 22) + 'px', fontSize: (size ?? 22) * 0.5 + 'px' }"
    aria-hidden="true"
  >{{ letter }}</span>
</template>
