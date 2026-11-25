<script setup lang="ts">
// A titled block on the canvas (Relay's "My Agents", "Approvals"): icon, title, a count chip, controls at the right,
// then its content flush to the edges (tables, lists) or padded (`pad`).
import type { Component } from 'vue'

defineProps<{ title: string; icon?: Component; count?: string | number; countTone?: 'signal' | 'neutral'; pad?: boolean; to?: string }>()
</script>

<template>
  <section class="min-w-0 overflow-hidden rounded-2xl border border-line bg-panel">
    <header class="flex min-h-15 flex-wrap items-center gap-x-3 gap-y-2 px-5 py-3">
      <component :is="icon" v-if="icon" class="size-[18px] shrink-0 text-ink" aria-hidden="true" />
      <h2 class="text-[17px] leading-tight font-semibold tracking-[-0.01em]">
        <RouterLink v-if="to" :to="to" class="hover:underline hover:decoration-line2 hover:underline-offset-4">{{ title }}</RouterLink>
        <template v-else>{{ title }}</template>
      </h2>
      <span v-if="count !== undefined && count !== ''"
        class="inline-flex h-6 items-center rounded-md px-2 text-[12.5px] font-medium tabular-nums"
        :class="countTone === 'signal' ? 'bg-signal/50 text-signal-ink dark:bg-signal/15' : 'border border-line bg-panel2 text-ink2'">{{ count }}</span>
      <slot name="title-extra" />
      <div v-if="$slots.actions" class="ml-auto flex flex-wrap items-center gap-1.5"><slot name="actions" /></div>
    </header>
    <div :class="pad ? 'px-5 pb-5' : ''"><slot /></div>
  </section>
</template>
