<script setup lang="ts">
import { ref } from 'vue';
import { plural } from './look';
import { useOpenMemory, useStream } from './streamContext';
import type { Recalled } from './stream';
import Icon from './Icon.vue';
import { Badge } from './ui/badge';

// What a turn was given of memory, under the message it answered (the server keeps it on the prompt, as `PromptData.recalled`):
// one quiet line, "Recalled 3 memories", that opens to each one's kind and words, as remit's context folds do. A memory of
// the agent's own opens in its Memory tab (where the drawer offers one); a crew member is also given the Assistant's, which
// say so. `card` is whose stream it is; `mine` sets it on the owner's side of the column, under his bubble.
const props = defineProps<{ recalled: Recalled[]; card: string; mine?: boolean }>();
const open = ref(false);
const stream = useStream();
const openMemory = useOpenMemory();
const KIND: Record<string, string> = { fact: 'Fact', preference: 'Preference', principle: 'Principle', lesson: 'Lesson', decision: 'Decision', intent: 'Intent', pitfall: 'Pitfall', procedure: 'Procedure' };
const own = (r: Recalled) => !r.holder || r.holder === props.card;
</script>

<template>
  <div class="recall" :class="{ mine }" data-recalled>
    <button type="button" class="recall-h" :aria-expanded="open" @click="open = !open">
      <Icon name="memory" :size="11" class="flex-none" />
      <span>Recalled {{ plural(recalled.length, 'memory', 'memories') }}</span>
      <Icon :name="open ? 'chevron-up' : 'chevron-down'" :size="11" class="flex-none" />
    </button>
    <ul v-if="open" class="recall-list" :aria-label="`What the turn was given of memory: ${recalled.length}`">
      <li v-for="r in recalled" :key="r.id" class="flex min-w-0 items-baseline gap-1.5" :data-recalled-memory="r.id">
        <Badge variant="quiet" class="flex-none">{{ KIND[r.kind] ?? r.kind }}</Badge>
        <button v-if="openMemory && own(r)" type="button" class="min-w-0 text-left text-text2 hover:text-text hover:underline [overflow-wrap:anywhere]" :aria-label="`Open in Memory: ${r.title}`" @click="openMemory(r.id)">{{ r.title }}</button>
        <span v-else class="min-w-0 text-text2 [overflow-wrap:anywhere]">{{ r.title }}</span>
        <span v-if="!own(r)" class="flex-none text-faint">· {{ stream.nameOf(r.holder) }}'s</span>
      </li>
    </ul>
  </div>
</template>

<style scoped>
/* A quiet line under the message, as the context's folds are: the memory mark, the count, a chevron; opened, each memory
   as its kind and words. On the owner's side it keeps to the right edge, under his bubble. */
.recall {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  margin-top: 3px;
  font-size: 11.5px;
  color: var(--mut);
}
.recall.mine {
  align-items: flex-end;
}
.recall-h {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  min-height: 24px;
  border-radius: 4px;
  cursor: pointer;
}
.recall-h:hover {
  color: var(--text2);
}
.recall-list {
  display: flex;
  flex-direction: column;
  gap: 4px;
  max-width: 85%;
  margin: 2px 0 0;
  padding: 6px 10px;
  list-style: none;
  border-radius: 8px;
  background: var(--panel2);
  text-align: left;
}
</style>
