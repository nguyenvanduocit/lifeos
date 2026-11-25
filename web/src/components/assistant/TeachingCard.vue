<script setup lang="ts">
import { computed, ref } from 'vue';
import type { Memory } from '../../core/generated/Memory';
import type { Teaching } from '../../core/generated/Teaching';
import { at, fullTime, plural, short } from './look';
import Ago from './Ago.vue';
import Icon from './Icon.vue';
import Tip from './Tip.vue';
import { Badge } from './ui/badge';
import { Button } from './ui/button';

// One teaching an agent's memory learned from, as its book lists them (MemoryBook), as remit's TeachingCard: what kind it
// is (what the owner taught in a turn, an overturn, a mark on a reply, or a review of what it already holds), whether the
// librarian has settled it, who taught it and when, what it came from, what was said (a few lines until asked for in full),
// what the librarian wrote of it or why nothing, and the memories it wrote, each resolved by `find` within the book and
// opened by `jump`; `landed` marks the card a link went to.
const props = defineProps<{ teaching: Teaching; find: (id: number) => Memory | undefined; landed: boolean }>();
const emit = defineEmits<{ jump: [id: number] }>();

const ROLE: Record<string, { label: string; icon: string }> = {
  teaching: { label: 'Taught', icon: 'comment' },
  review: { label: 'Review', icon: 'repeat' },
};
const t = computed(() => props.teaching);
const role = computed(() => ROLE[t.value.role] ?? { label: t.value.role, icon: 'dot' });
const settled = computed(() => t.value.status !== 'open');
const teacher = computed(() => (t.value.teacher === 'owner' ? 'you' : ''));
const written = computed(() => t.value.written.map((id) => ({ id, memory: props.find(id) })));
const titleOf = (m: Memory) => m.title.trim() || short(m.body, 80) || `#${m.id}`;
// What was said shows a few lines; Read it opens the rest, offered when there is more than those lines hold.
const reading = ref(false);
const long = computed(() => t.value.text.split('\n').length > 4 || t.value.text.length > 320);
</script>

<template>
  <li class="teaching flex flex-col gap-1.5 rounded-lg px-3 py-2.5" tabindex="-1" :data-teaching="teaching.id" :data-state="settled ? 'settled' : 'waiting'">
    <div class="flex min-w-0 flex-wrap items-center gap-x-3 gap-y-1 text-xs text-mut">
      <Badge v-if="landed" variant="info" class="flex-none" data-landed><Icon name="arrow-right" :size="11" />Opened from a link</Badge>
      <Badge variant="quiet" class="flex-none" data-role><Icon :name="role.icon" :size="11" />{{ role.label }}</Badge>
      <Badge v-if="!settled" variant="info" class="flex-none" data-teaching-state><Icon name="clock" :size="11" />Waiting for the librarian</Badge>
      <Badge v-else variant="quiet" class="flex-none" data-teaching-state><Icon name="check" :size="11" />Settled</Badge>
      <span v-if="teacher" class="min-w-0 [overflow-wrap:anywhere]" data-teacher>taught by <span class="text-text2">{{ teacher }}</span></span>
      <span>given <Tip :text="fullTime(at(teaching.created))"><time class="rounded-sm" :datetime="at(teaching.created)"><Ago :at="at(teaching.created)" /></time></Tip></span>
      <span v-if="teaching.settled">settled <Tip :text="fullTime(at(teaching.settled))"><time class="rounded-sm" :datetime="at(teaching.settled)"><Ago :at="at(teaching.settled)" /></time></Tip></span>
    </div>
    <p class="m-0 flex min-w-0 items-baseline gap-1.5 text-xs text-mut" data-source>
      <Icon class="mt-0.5 flex-none self-start" name="memory" :size="11" />
      <span class="min-w-0 [overflow-wrap:anywhere]">{{ teaching.source || 'no source named' }}</span>
    </p>
    <template v-if="teaching.text.trim()">
      <p class="said m-0 border-l-2 border-line2 pl-2.5 text-meta text-text2 [overflow-wrap:anywhere]" :class="{ clipped: long && !reading }" data-text>{{ teaching.text }}</p>
      <Button v-if="long" class="self-start" variant="link" size="text" :aria-expanded="reading" :aria-label="`${reading ? 'Hide' : 'Read it'}: what was taught`" data-show-text @click="reading = !reading">{{ reading ? 'Hide' : 'Read it' }}<Icon :name="reading ? 'chevron-up' : 'chevron-down'" :size="11" /></Button>
    </template>
    <p v-if="teaching.note.trim()" class="m-0 flex min-w-0 items-baseline gap-1.5 text-xs text-text2" data-note>
      <Icon class="mt-0.5 flex-none self-start text-mut" name="memory" :size="11" /><span class="min-w-0 [overflow-wrap:anywhere]"><span class="text-mut">The librarian: </span>{{ teaching.note }}</span>
    </p>
    <div v-if="written.length" class="flex min-w-0 flex-col gap-0.5 text-xs" data-written>
      <span class="text-mut">Wrote {{ plural(written.length, 'memory', 'memories') }}</span>
      <ul class="m-0 flex list-none flex-col gap-0.5 p-0">
        <li v-for="w in written" :key="w.id" class="flex min-w-0 items-baseline gap-1.5">
          <Icon class="mt-0.5 flex-none self-start text-mut" name="arrow-right" :size="11" />
          <Button v-if="w.memory" variant="link" size="text" :aria-label="`Go to the memory it wrote: ${titleOf(w.memory)}`" @click="emit('jump', w.id)">{{ titleOf(w.memory) }}</Button>
          <span v-else class="min-w-0 text-mut [overflow-wrap:anywhere]">a memory this book does not hold (<span class="mono">#{{ w.id }}</span>)</span>
        </li>
      </ul>
    </div>
    <span v-else-if="settled" class="text-xs text-mut" data-written>Wrote nothing</span>
  </li>
</template>

<style scoped>
.teaching {
  background: var(--panel2);
  scroll-margin-top: calc(var(--memory-bar, 0px) + 8px);
}
/* The card a link went to holds the focus on a lifted surface, beside its "Opened from a link" badge. */
.teaching:focus {
  outline: none;
  background: var(--panel3);
}
/* What was said keeps its line breaks, a few lines of it until Read it opens the rest. */
.said {
  white-space: pre-wrap;
}
.said.clipped {
  display: -webkit-box;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 4;
  overflow: hidden;
}
</style>
