<script setup lang="ts">
import { computed } from 'vue';
import { agentLetter, agentStyle } from './look';

// An agent as a lettered square, `size` px wide (rounded in proportion), as remit draws one: the Assistant in its violet,
// a crew member in a colour from its id (never from its place in a list). `label` is what a screen reader says in its place
// where the words beside it do not name it.
const props = withDefaults(defineProps<{ id: string; name: string; size?: number; label?: string }>(), { size: 28 });
const box = computed(() => ({ width: props.size + 'px', height: props.size + 'px', borderRadius: Math.round(props.size / 4) + 'px', fontSize: (props.size < 26 ? 10 : 11.5) + 'px' }));
</script>

<template>
  <div class="badge" :style="{ ...agentStyle(id), ...box }" :role="label ? 'img' : undefined" :aria-label="label" :aria-hidden="label ? undefined : 'true'">{{ agentLetter(name) }}</div>
</template>

<style scoped>
.badge {
  display: flex;
  flex: none;
  align-items: center;
  justify-content: center;
  font-weight: 700;
}
</style>
