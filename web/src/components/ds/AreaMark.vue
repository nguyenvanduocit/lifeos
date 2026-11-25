<script setup lang="ts">
// An area's identity, as small as it gets: an 8px square in its hue (docs/design.md, principle 5).
defineProps<{ area: string; size?: number }>()
</script>

<template>
  <span :data-tone="area" class="inline-block shrink-0 rounded-[2px] bg-tone" :style="{ width: (size ?? 8) + 'px', height: (size ?? 8) + 'px' }" aria-hidden="true" />
</template>
