<script setup lang="ts">
// A few figures in one soft bar, each an icon, the value, what it counts and a change (Relay's
// "38 hrs saved by agents +12%"). Every figure carries its frame (docs/design.md, principle 2).
import type { Component } from 'vue'
import Delta from './Delta.vue'

defineProps<{ items: { key: string; icon: Component; value: string; label: string; delta?: string; deltaTone?: 'good' | 'bad' | 'flat'; to?: string; urgent?: boolean }[] }>()
</script>

<template>
  <div class="inline-flex max-w-full flex-wrap items-center rounded-2xl border border-line bg-panel2 px-1.5 py-1.5">
    <template v-for="(it, i) in items" :key="it.key">
      <span v-if="i" class="mx-1 hidden h-6 w-px bg-line2 sm:block" aria-hidden="true" />
      <component :is="it.to ? 'RouterLink' : 'div'" :to="it.to"
        class="flex min-h-10 flex-wrap items-center gap-x-2.5 gap-y-1 rounded-xl px-3 py-1 transition-colors"
        :class="it.to ? 'hover:bg-panel focus-visible:outline-2 focus-visible:outline-ring' : ''">
        <component :is="it.icon" class="size-[18px] shrink-0" :class="it.urgent ? 'text-critical' : 'text-ink2'" aria-hidden="true" />
        <span class="text-[19px] leading-none font-semibold tracking-tight tabular-nums">{{ it.value }}</span>
        <span class="text-[14px] whitespace-nowrap text-ink2">{{ it.label }}</span>
        <Delta v-if="it.delta" :tone="it.deltaTone" class="whitespace-nowrap">{{ it.delta }}</Delta>
      </component>
    </template>
  </div>
</template>
