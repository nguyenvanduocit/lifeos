<script setup lang="ts">
// A list's column heads on a soft band (Relay): an icon and a word per column, laid out by the caller's grid.
import type { Component } from 'vue'

// `grid` is the columns on a wide container, `gridSm` (default: the first column and one more) below 44rem; cells that
// a narrow container drops carry `hidden @min-[44rem]:flex` in their class.
defineProps<{ cols: { label: string; icon?: Component; class?: string }[]; grid: string; gridSm?: string }>()
</script>

<template>
  <div role="row" class="grid items-center gap-x-4 border-y border-line bg-panel2 px-5 py-2.5 text-[13px] text-ink2" :style="{ '--cols': grid, '--cols-sm': gridSm ?? 'minmax(0,1fr) auto' }" data-cols>
    <div v-for="c in cols" :key="c.label" role="columnheader" class="flex min-w-0 items-center gap-2" :class="c.class">
      <component :is="c.icon" v-if="c.icon" class="size-4 shrink-0 text-ink3" aria-hidden="true" />
      <span class="truncate">{{ c.label }}</span>
    </div>
  </div>
</template>
