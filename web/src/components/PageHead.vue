<script setup lang="ts">
defineProps<{ title: string; sub?: string }>()
</script>

<template>
  <header class="mb-4 flex flex-wrap items-end gap-3">
    <div>
      <h1 class="text-2xl font-semibold tracking-tight">{{ title }}</h1>
      <p v-if="sub" class="text-sm text-ink2">{{ sub }}</p>
    </div>
    <div class="ml-auto flex flex-wrap gap-1.5"><slot /></div>
  </header>
</template>
