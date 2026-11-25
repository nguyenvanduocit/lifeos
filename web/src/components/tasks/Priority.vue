<script setup lang="ts">
// A task's priority as a coloured glyph and a word (never a solid pill); normal priority shows nothing.
import { ChevronDown, ChevronsUp } from '@lucide/vue'

defineProps<{ priority: string | null; compact?: boolean }>()
</script>

<template>
  <span v-if="priority === 'high'" class="inline-flex items-center gap-1 text-[13px] font-medium text-critical" title="High priority">
    <ChevronsUp class="size-4" aria-hidden="true" /><span :class="compact ? 'sr-only' : ''">High</span>
  </span>
  <span v-else-if="priority === 'low'" class="inline-flex items-center gap-1 text-[13px] text-ink3" title="Low priority">
    <ChevronDown class="size-4" aria-hidden="true" /><span :class="compact ? 'sr-only' : ''">Low</span>
  </span>
  <span v-else class="text-[13px] text-ink3" aria-label="Normal priority">—</span>
</template>
