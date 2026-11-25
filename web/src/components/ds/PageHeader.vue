<script setup lang="ts">
import PageActions from './PageActions.vue'

defineProps<{ title: string; eyebrow?: string; sub?: string }>()
</script>

<template>
  <h1 class="sr-only">{{ title }}</h1>
  <PageActions v-if="$slots.default"><slot /></PageActions>
  <div v-if="$slots.eyebrow" class="mb-2 flex items-center gap-2 text-sm text-ink2"><slot name="eyebrow" /></div>
  <div v-if="$slots.strip" class="mb-4 empty:hidden"><slot name="strip" /></div>
</template>
