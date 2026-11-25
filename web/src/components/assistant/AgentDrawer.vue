<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import { useQueryRef } from '../../nav';
import { useRead } from '../../data';
import { provideOpenMemory } from './streamContext';
import { useStickToBottom } from './useStickToBottom';
import type { AgentStanding } from '../../core/generated/AgentStanding';
import type { Card } from '../../core/generated/Card';
import type { EventRead } from '../../core/generated/EventRead';
import type { Pending } from '../../assistant';
import AgentPlan from './AgentPlan.vue';
import CardActivity from './CardActivity.vue';
import CardComposer from './CardComposer.vue';
import CardHeader from './CardHeader.vue';
import CardSession from './CardSession.vue';
import CardTelemetry from './CardTelemetry.vue';
import ContextDetails from './ContextDetails.vue';
import EmptyState from './EmptyState.vue';
import HandoffStrip from './HandoffStrip.vue';
import JumpToLatest from './JumpToLatest.vue';
import MemoryBook from './MemoryBook.vue';
import PendingSends from './PendingSends.vue';
import QuestionCallout from './QuestionCallout.vue';

// An agent as the owner opens it, as remit's drawer is: who it is, the panel the address names (`panelKey`, its activity
// unless named) and the box to message it. The context meter opens the Session panel at what fills the context. Whose agent it is is the caller's:
// it hands over the agent's stream, standing and the owner's sends on their way (`pending`), and puts its own buttons in
// `actions` and the agent's name in `title`; `earlier`, when the stream goes back further than the events handed over,
// reads the page before them. The Decisions panel is the caller's (`decisions`, offered when `decided`). What the agent waits
// on the owner for (its questions, `agent.waiting`) is answered once, pinned under the panel whichever panel is open, so the
// answer never depends on how far the stream is scrolled; the header's "Waiting on your answer" takes the keyboard there.
// An agent with a memory (`agent.remembers`) shows it in the Memory panel (MemoryBook), where a memory the stream says a
// turn was given opens.
const props = defineProps<{
  card: Card;
  name: string;
  agent: AgentStanding;
  events: readonly EventRead[];
  pending: readonly Pending[];
  panelKey: string;
  earlier?: () => void;
  decided?: boolean;
  decisionsNew?: number;
}>();
const tab = useQueryRef(props.panelKey, 'activity', ['activity', 'decisions', 'plan', 'session', 'memory'] as const);
const revealContext = ref(false);
function goToContext() {
  tab.value = 'session';
  revealContext.value = true;
}

// The body follows the newest activity while the owner is at its bottom, and stays put while they read older blocks, a
// button counting what arrived meanwhile and taking him back (JumpToLatest). Only the activity is a feed: each panel opens
// at its start, the activity at its newest line.
const body = ref<HTMLElement | null>(null);
const feed = computed(() => (tab.value === 'activity' ? body.value : null));
const { stuck, unseen, onScroll, follow } = useStickToBottom(
  feed,
  () => props.events.at(-1)?.event.id + ':' + (props.events.at(-1)?.event.text.length ?? 0) + ':' + props.agent.waiting.length + ':' + props.pending.map((p) => p.status).join(),
  () => props.events,
);
watch(tab, (t) => {
  if (t === 'activity') void follow();
  else if (body.value) body.value.scrollTop = 0;
});
watch(() => props.card.id, () => void follow(), { immediate: true });

// The questions it waits on, as the server lists them (read again whenever the data changes), in the order it asked.
const { data: questions } = useRead('list_questions', () => ({ waiting: true }));
const waiting = computed(() => (questions.value ?? []).filter((q) => props.agent.waiting.includes(q.id)).sort((a, b) => a.id - b.id));
const pinned = ref<HTMLElement | null>(null);
const goToWaiting = () => pinned.value?.querySelector<HTMLElement>('button, input')?.focus();

// A memory a turn was given opens in the Memory panel, at its card.
const landOn = ref<number | null>(null);
provideOpenMemory((id) => {
  landOn.value = id;
  tab.value = 'memory';
});
</script>

<template>
  <aside class="flex min-h-0 min-w-0 flex-1 flex-col bg-bg2" :aria-label="name">
    <CardHeader v-model:tab="tab" :card="card" :agent="agent" :decided="decided" :decisions-new="decisionsNew" @context="goToContext" @needs="goToWaiting"><template #title><slot name="title" /></template><template #actions><slot name="actions" /></template></CardHeader>
    <div class="relative flex min-h-0 flex-1 flex-col">
      <div ref="body" class="flex-1 overflow-auto px-5 py-4 [contain:content]" @scroll="onScroll">
        <slot v-if="tab === 'decisions'" name="decisions" />
        <CardActivity v-if="tab === 'activity'" :key="card.id" :card="card" :events="events" :agent="agent" :earlier="earlier" />
        <PendingSends v-if="tab === 'activity'" class="mt-3" :card="card.id" :pending="[...pending]" />
        <template v-if="tab === 'plan'">
          <AgentPlan v-if="card.plan.length" :plan="card.plan" />
          <EmptyState v-else>No plan yet.</EmptyState>
        </template>
        <template v-if="tab === 'session'">
          <CardSession :card="card" />
          <ContextDetails v-model:reveal="revealContext" class="mt-4" :card="card" />
          <CardTelemetry :card="card" />
        </template>
        <template v-if="tab === 'memory'">
          <MemoryBook v-if="agent.remembers" v-model:land="landOn" :holder="card.id" :who="name" :view-key="`${panelKey}Memory`" />
          <EmptyState v-else>{{ name }} keeps no memory of its own.</EmptyState>
        </template>
      </div>
      <JumpToLatest v-if="tab === 'activity' && !stuck" class="absolute right-4 bottom-3" :unseen="unseen" @click="follow" />
    </div>
    <div v-if="waiting.length" ref="pinned" class="flex max-h-[45%] flex-none flex-col gap-2 overflow-auto border-t border-line px-3 py-2.5" data-pinned-asks>
      <QuestionCallout v-for="q in waiting" :key="q.id" :question="q" />
    </div>
    <HandoffStrip :card="card.id" />
    <CardComposer :card="card" :name="name" :agent="agent" @sent="follow" />
  </aside>
</template>
