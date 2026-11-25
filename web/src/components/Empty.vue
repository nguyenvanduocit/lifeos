<script setup lang="ts">
import AskChip from './AskChip.vue'

defineProps<{ text: string; words?: string; label?: string }>()
</script>

<template>
  <div class="flex flex-col items-start gap-2 py-2 text-sm text-ink3">
    <span>{{ text }}</span>
    <AskChip v-if="words" :words="words" :label="label" />
  </div>
</template>
