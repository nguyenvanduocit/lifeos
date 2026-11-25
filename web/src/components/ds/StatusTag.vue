<script setup lang="ts">
// A state as a dot and a word on a soft tint (Relay's "● Running"): never a solid pill.
// tone: good | warning | critical | info | signal | neutral, or an area. `pulse` for what is happening now.
defineProps<{ tone?: string; pulse?: boolean; dot?: boolean }>()
</script>

<template>
  <span :data-tone="tone ?? 'neutral'"
    class="inline-flex h-6 shrink-0 items-center gap-1.5 rounded-full bg-tone-wash px-2.5 text-[12.5px] leading-none font-medium whitespace-nowrap text-tone-ink">
    <span v-if="dot !== false" class="relative flex size-1.5">
      <span v-if="pulse" class="absolute inline-flex size-full rounded-full bg-tone opacity-60 motion-safe:animate-ping" />
      <span class="relative inline-flex size-1.5 rounded-full bg-tone" />
    </span>
    <slot />
  </span>
</template>
