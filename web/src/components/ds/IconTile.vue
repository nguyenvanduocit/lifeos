<script setup lang="ts">
// An object's mark in a list: its icon on a soft square of its tone (Relay's agent tiles).
import type { Component } from 'vue'

defineProps<{ icon: Component; tone?: string; size?: 'sm' | 'md' | 'lg' }>()
</script>

<template>
  <span :data-tone="tone"
    class="grid shrink-0 place-items-center bg-tone-wash text-tone-ink"
    :class="size === 'lg' ? 'size-11 rounded-xl' : size === 'sm' ? 'size-7 rounded-md' : 'size-9 rounded-lg'">
    <component :is="icon" :class="size === 'lg' ? 'size-5' : size === 'sm' ? 'size-3.5' : 'size-4'" aria-hidden="true" />
  </span>
</template>
