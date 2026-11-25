<script setup lang="ts">
// A person's initials on a colour that stays theirs (by name, never by place in a list).
import { computed } from 'vue'

const props = defineProps<{ name: string; size?: number }>()
const HUES = [12, 32, 145, 175, 200, 225, 260, 290, 330]
const hue = computed(() => {
  let h = 0
  for (const ch of props.name) h = (h * 31 + ch.charCodeAt(0)) >>> 0
  return HUES[h % HUES.length]
})
const initials = computed(() => props.name.split(/\s+/).filter(Boolean).slice(0, 2).map((w) => w[0]).join('').toUpperCase())
</script>

<template>
  <span
    class="inline-grid shrink-0 place-items-center rounded-full font-bold"
    :style="{
      width: (size ?? 36) + 'px', height: (size ?? 36) + 'px', fontSize: (size ?? 36) * 0.36 + 'px',
      background: `linear-gradient(135deg, oklch(0.72 0.13 ${hue}), oklch(0.55 0.15 ${hue + 25}))`, color: 'white',
    }"
    aria-hidden="true"
  >{{ initials }}</span>
</template>
