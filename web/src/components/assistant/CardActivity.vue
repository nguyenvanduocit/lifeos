<script setup lang="ts">
import { computed } from 'vue';
import { blocks, toolOf } from './stream';
import { useNewest } from './useNewest';
import { doingText } from './look';
import type { AgentStanding } from '../../core/generated/AgentStanding';
import type { Card } from '../../core/generated/Card';
import type { EventRead } from '../../core/generated/EventRead';
import StreamBlocks from './StreamBlocks.vue';
import Callout from './Callout.vue';
import EmptyState from './EmptyState.vue';
import LiveDot from './LiveDot.vue';
import ShowMore from './ShowMore.vue';

// The agent's stream: what it read, thought, ran and said, a typing line while it works, and last, the error it stopped on.
// The typing line says what the server says it is doing (`agent`). One bar above it shows older activity: first what is
// loaded but not drawn, then, once all of that shows, the page before it from the server (`earlier`, when the stream goes
// back further than `events`).
const props = defineProps<{ card: Card; events: readonly EventRead[]; agent: AgentStanding; earlier?: () => void }>();
const { shown, older, more } = useNewest(() => blocks(props.events));
const showOlder = () => (older.value ? more() : void props.earlier?.());
const busy = computed(() => props.card.status === 'working');
// The feed already shows a running tool (its card, with a spinner) and a thought being written (its Thinking block, open),
// so the typing line is only for the rest.
const feedShowsIt = computed(() => {
  const last = props.events[props.events.length - 1];
  if (last?.event.kind === 'tool') return toolOf(last).status === 'running';
  return last?.event.kind === 'thought' && busy.value;
});
</script>

<template>
  <ShowMore v-if="older || earlier" class="mb-2.5" @click="showOlder">{{ older ? `Show older (${older})` : 'Show older activity' }}</ShowMore>
  <StreamBlocks :blocks="shown" :live="busy" />
  <div v-if="(busy || card.status === 'starting') && !feedShowsIt" class="typing" :class="doingText(agent.doing_tone) || 'text-mut'"><LiveDot :live="agent.doing_tone === 'busy'" />{{ agent.doing }}</div>
  <EmptyState v-if="!shown.length">No activity yet: what the agent does shows here as it works.</EmptyState>
  <Callout v-if="card.error" tone="error" class="mt-3">
    <template #title>{{ card.name }} error</template>
    <pre>{{ card.error }}</pre>
  </Callout>
</template>

<style scoped>
/* The agent is at work: its dot pings beside what it is doing, in the colour of that, as in its header. */
.typing {
  display: flex;
  align-items: center;
  gap: 8px;
  margin: 10px 0 4px;
  font-size: 12px;
}
</style>
