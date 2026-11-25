<script setup lang="ts">
// A habit's last days as small squares, oldest first: ink when done, grey when not. `fluid` shares the width it is
// given (a table cell) instead of a fixed size per square.
defineProps<{ values: number[]; size?: 'sm' | 'md'; fluid?: boolean }>()
</script>

<template>
  <span :class="fluid ? 'flex w-full max-w-[22rem] gap-[2px]' : 'flex gap-[3px]'" role="img"
    :aria-label="`${values.filter((v) => v > 0).length} of the last ${values.length} days done`">
    <span v-for="(v, i) in values" :key="i"
      :class="[
        fluid ? 'aspect-square max-w-3 min-w-0 flex-1 rounded-[2px]' : size === 'sm' ? 'size-2.5 rounded-[3px]' : 'size-3.5 rounded-[4px]',
        v > 0 ? 'bg-ink/85' : 'bg-panel3',
      ]" />
  </span>
</template>
