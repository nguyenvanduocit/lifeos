<script setup lang="ts">
import { computed } from 'vue';
import { useRead } from '../../data';
import type { Handoff } from '../../core/generated/Handoff';
import { fullTime } from './look';
import Ago from './Ago.vue';
import Tip from './Tip.vue';

// What one agent asked or handed other agents that is still out, and what was handed to it (docs/adr/0002), pinned under
// its stream as its questions are: who has what, how it stands, since when, and what lifeos's health check saw (quiet for a
// while, handed again). Read again whenever the data changes; a hand-off leaves the strip once it closes, and its result
// reaches the sender's stream as one message.
const props = defineProps<{ card: string }>();
const { data } = useRead('list_handoffs', () => ({ open: true, agent: props.card }));
const rows = computed(() => (data.value ?? []).filter((h) => h.kind !== 'news').sort((a, b) => a.id - b.id));
const KIND: Record<Handoff['kind'], string> = { ask: 'question', task: 'task', news: 'news' };
const STATE: Record<Handoff['status'], string> = {
  queued: 'queued',
  working: 'working',
  waiting: 'waiting on what it handed on',
  done: 'done',
  failed: 'failed',
  cancelled: 'cancelled',
};
const firstLine = (text: string) => text.split('\n').find((l) => l.trim())?.trim() ?? '';
</script>

<template>
  <section v-if="rows.length" class="flex max-h-[30%] flex-none flex-col gap-1 overflow-auto border-t border-line px-3 py-2" aria-label="Hand-offs" data-handoffs>
    <h4 class="m-0 flex items-baseline text-[11px] font-medium tracking-wide text-mut uppercase">Hand-offs<RouterLink to="/talk" class="ml-auto normal-case tracking-normal hover:underline">All team talk</RouterLink></h4>
    <RouterLink v-for="h in rows" :key="h.id" :to="{ path: '/talk', query: { h: h.id } }" class="flex min-w-0 items-baseline gap-2 rounded-sm text-[12px] hover:bg-bg2" :data-handoff="h.id" :data-status="h.status" :title="`Open #${h.id} in Team talk`">
      <span class="flex-none text-mut">#{{ h.id }}</span>
      <span class="min-w-0 flex-1 truncate">
        <template v-if="h.sender === card">→ <b class="font-medium">{{ h.target_name }}</b></template>
        <template v-else>← <b class="font-medium">{{ h.sender_name }}</b></template>
        <span class="text-mut"> · {{ KIND[h.kind] }} · {{ STATE[h.status] }}</span>
        <span v-if="h.note" class="text-orange-text"> · {{ h.note }}</span>
        <span class="text-mut"> — {{ firstLine(h.text) }}</span>
      </span>
      <Tip :text="fullTime(h.updated)"><time class="flex-none text-[11px] text-mut" :datetime="h.updated"><Ago :at="h.updated" /></time></Tip>
    </RouterLink>
  </section>
</template>
