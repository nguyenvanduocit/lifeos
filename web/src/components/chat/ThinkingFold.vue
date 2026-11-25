<script setup lang="ts">
import { computed } from 'vue'
import Markdown from '../Markdown.vue'

const props = defineProps<{ text: string; live?: boolean }>()
const preview = computed(() => props.text.replace(/\s+/g, ' ').slice(0, 100))
</script>

<template>
  <details class="group text-xs" :open="live">
    <summary class="flex cursor-pointer list-none items-center gap-2 py-0.5">
      <span class="shrink-0 font-semibold whitespace-nowrap text-violet-text">✦ Suy nghĩ</span>
      <span class="truncate text-ink3 group-open:hidden">{{ preview }}</span>
    </summary>
    <Markdown :source="text" class="mt-1 border-l-2 border-violet-line pl-3 text-[13px] text-ink2" />
  </details>
</template>
