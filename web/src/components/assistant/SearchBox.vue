<script setup lang="ts">
import Icon from './Icon.vue';

// A search over a list, wherever one is narrowed by what the owner types (an agent's context): the search icon in front, a
// clear button once something is typed, and Esc clears it. `label` names it for a screen reader. (remit's is built on its
// input group, which the panel does not carry; this is the same control drawn plainly.)
defineProps<{ placeholder: string; label: string }>();
const query = defineModel<string>({ required: true });
</script>

<template>
  <div class="search flex items-center gap-1.5">
    <Icon class="flex-none text-mut" name="search" :size="13" />
    <input v-model="query" class="min-w-0 flex-1 bg-transparent text-[12.5px] text-text outline-none placeholder:text-faint" :placeholder="placeholder" :aria-label="label" autocomplete="off" @keydown.esc="query = ''" />
    <button v-if="query" type="button" class="hit flex-none rounded-[4px] text-mut hover:text-text" aria-label="Clear search" @click="query = ''"><Icon name="x" :size="12" /></button>
  </div>
</template>

<style scoped>
.search {
  height: var(--control-sm, 28px);
  padding: 0 8px;
  border: 1px solid var(--line2);
  border-radius: 6px;
  background: var(--panel);
}
.search:focus-within {
  border-color: var(--blue-line);
}
</style>
