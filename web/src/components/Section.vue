<script setup lang="ts">
import { ChevronRight } from '@lucide/vue'

defineProps<{ title: string; to?: string; count?: number }>()
</script>

<template>
  <section class="rounded-xl border border-line bg-panel p-4">
    <header class="mb-3 flex items-center gap-2">
      <h2 class="text-sm font-semibold text-ink">{{ title }}</h2>
      <span v-if="count" class="rounded-full bg-panel2 px-1.5 text-xs text-ink2">{{ count }}</span>
      <div class="ml-auto flex items-center gap-1">
        <slot name="actions" />
        <RouterLink v-if="to" :to="to" class="flex items-center text-xs text-ink3 hover:text-ink" aria-label="Xem tất cả">
          Xem <ChevronRight :size="14" />
        </RouterLink>
      </div>
    </header>
    <slot />
  </section>
</template>
