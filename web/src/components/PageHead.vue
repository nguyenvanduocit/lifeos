<script setup lang="ts">
defineProps<{ title: string; sub?: string }>()
</script>

<template>
  <header class="mb-5 flex flex-wrap items-center justify-between gap-x-5 gap-y-3 border-b border-line pb-4">
    <div class="min-w-0">
      <h1 class="font-display text-4xl leading-none tracking-tight sm:text-[2.75rem]">{{ title }}</h1>
      <p v-if="sub" class="mt-1.5 max-w-prose text-sm leading-relaxed text-ink2">{{ sub }}</p>
    </div>
    <div v-if="$slots.default" class="flex flex-wrap gap-2"><slot /></div>
  </header>
</template>
