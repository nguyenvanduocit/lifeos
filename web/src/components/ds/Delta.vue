<script setup lang="ts">
// A change beside a figure: "+12%" on a soft lime (up and good), soft peach (bad), grey (flat or unknown).
const props = defineProps<{ tone?: 'good' | 'bad' | 'flat' }>()
</script>

<template>
  <span class="inline-flex h-5 items-center rounded-md px-1.5 text-[12px] leading-none font-semibold tabular-nums"
    :class="props.tone === 'bad' ? 'bg-critical/12 text-critical' : props.tone === 'flat' ? 'bg-panel3 text-ink2' : 'bg-signal/45 text-signal-ink dark:bg-signal/15'">
    <slot />
  </span>
</template>
