<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, watch } from 'vue';
import { useRouter } from 'vue-router';
import { ASSISTANT, agentState, loadOlder, loadStream, panel, restart } from '../assistant';
import { useAssistant } from '../nav';
import { useRead } from '../data';
import { provideStream } from './assistant/streamContext';
import AgentDrawer from './assistant/AgentDrawer.vue';
import AgentPicker from './assistant/AgentPicker.vue';
import Callout from './assistant/Callout.vue';
import EmptyState from './assistant/EmptyState.vue';
import Icon from './assistant/Icon.vue';
import Spinner from './assistant/Spinner.vue';
import Tip from './assistant/Tip.vue';
import { Button } from './assistant/ui/button';
import { TooltipProvider } from './assistant/ui/tooltip';
import CardDecisions from './assistant/CardDecisions.vue';

// The Assistant, and each crew member, in the column beside whatever page is open, as remit's Assistant panel is
// (docs/adr/0001): an agent's drawer over its card, its tab the address's ?assistantPanel=. It wears remit's look, scoped
// to it (`.remit`, panel.css). Who it talks to is picked in its header. The agent's stream is read when it is first shown.
// Start over is offered when the server says it can be asked. The box takes the focus as it opens, and Escape inside the
// panel closes it and gives the focus back to the top bar's button.
const { close } = useAssistant();
const router = useRouter();
const { data: crew } = useRead('list_crew');
// What the Assistant left to the owner, counted on its Decisions tab.
const { data: questions } = useRead('list_questions', () => ({ waiting: false }));
const leftToYou = computed(() => (questions.value ?? []).filter((q) => q.status === 'left').length);
const active = computed(() => (crew.value ?? []).filter((m) => m.status === 'active').map((m) => ({ id: m.id, name: m.name, role: m.role ?? '' })));
const nameOf = (id: string) => (id === ASSISTANT ? 'Assistant' : (crew.value?.find((m) => m.id === id)?.name ?? id));
provideStream({ nameOf, standingOf: (id) => agentState(id).view?.standing });

const a = computed(() => agentState(panel.agent));
const name = computed(() => nameOf(panel.agent));
watch(
  () => panel.agent,
  (id) => {
    if (!agentState(id).loaded) void loadStream(id);
  },
  { immediate: true },
);
const earlier = computed(() => (a.value.older ? () => void loadOlder(panel.agent) : undefined));

const root = ref<HTMLElement | null>(null);
void nextTick(() => root.value?.querySelector<HTMLElement>('textarea, input')?.focus());
// However it closes (Esc, its X, the address), a focus that was in the panel, or already let go of (the box blurs on Esc
// before the panel hears it), goes back to the top bar's button that opened it.
onBeforeUnmount(() => {
  const at = document.activeElement;
  if (!at || at === document.body || root.value?.contains(at)) document.querySelector<HTMLElement>('[data-assistant-toggle]')?.focus();
});
function manageCrew() {
  void router.push('/crew');
}
</script>

<template>
  <TooltipProvider :delay-duration="300">
    <div ref="root" class="remit flex h-full min-h-0 min-w-0 flex-1 bg-bg2" data-assistant @keydown.esc="close()">
      <AgentDrawer v-if="a.view" :card="a.view.card" :name="name" :agent="a.view.standing" :events="a.events" :pending="a.pending" panel-key="assistantPanel" :earlier="earlier" :decided="panel.agent === ASSISTANT" :decisions-new="leftToYou">
        <template #title><AgentPicker :current="{ id: panel.agent, name }" :crew="active" @manage="manageCrew" /></template>
        <template #actions>
          <Tip v-if="a.view.standing.asks.includes('restart')" text="Clear this conversation and close its session: your next message opens a new one, which reads a recap of it">
            <Button variant="outline" size="sm" @click="restart(panel.agent)">Start over</Button>
          </Tip>
          <Tip text="Close (Esc). It keeps working.">
            <Button variant="ghost" size="icon-sm" :aria-label="`Close ${name}`" @click="close()"><Icon name="x" /></Button>
          </Tip>
        </template>
        <template #decisions><CardDecisions /></template>
      </AgentDrawer>
      <div v-else class="flex flex-1 flex-col gap-3 px-5 py-4">
        <div class="flex items-center justify-between">
          <AgentPicker :current="{ id: panel.agent, name }" :crew="active" @manage="manageCrew" />
          <Button variant="ghost" size="icon-sm" :aria-label="`Close ${name}`" @click="close()"><Icon name="x" /></Button>
        </div>
        <Callout v-if="a.error" tone="error"><template #title>Could not reach {{ name }}</template><pre>{{ a.error }}</pre><Button variant="outline" size="sm" @click="loadStream(panel.agent)">Try again</Button></Callout>
        <EmptyState v-else class="flex items-center gap-2"><Spinner :size="12" />Loading…</EmptyState>
      </div>
      <p v-if="a.view && a.error" role="alert" class="sr-only">{{ a.error }}</p>
    </div>
  </TooltipProvider>
</template>
