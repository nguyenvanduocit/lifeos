<script setup lang="ts">
import { doingText } from './look';
import type { AgentStanding } from '../../core/generated/AgentStanding';
import type { Card } from '../../core/generated/Card';
import AgentStandingBadge from './AgentStandingBadge.vue';
import StandingBadge from './StandingBadge.vue';
import ContextMeter from './ContextMeter.vue';
import LiveDot from './LiveDot.vue';
import PageHead from './PageHead.vue';
import Tip from './Tip.vue';
import { Badge } from './ui/badge';
import { TabsTrigger } from './ui/tabs';

// Who the agent is and what it is doing (`agent`: where the server says it stands), and the drawer's panels to pick from,
// with how full its context is at the end of the panels' row (the meter asks for what fills it: `context`). It is a
// PageHead, as remit's is. The place it is shown in names the agent (`title`: the panel's picker) and adds its own buttons
// (`actions`: start over, close) at the end of the first row. The Decisions panel is offered for the Assistant (`decided`),
// counting what waits on the owner in it (`decisionsNew`). An agent with open questions to the owner says so first, and its
// mark asks to be taken to them, pinned under the stream (`needs`). The Memory panel is offered for an agent that remembers.
defineProps<{ card: Card; agent: AgentStanding; decided?: boolean; decisionsNew?: number }>();
defineEmits<{ context: []; needs: [] }>();
const tab = defineModel<string>('tab', { required: true });
</script>

<template>
  <PageHead :title="card.name" :tab="tab" @update:tab="tab = $event">
    <template #title><slot name="title" /></template>
    <template #standing>
      <Tip v-if="agent.waiting.length" text="What it waits for is pinned under the stream">
        <button type="button" class="flex min-w-0 flex-none" aria-label="Waiting on your answer: answer it" data-waiting @click="$emit('needs')">
          <StandingBadge class="min-w-0 truncate" variant="needs" icon="clock">Waiting on your answer</StandingBadge>
        </button>
      </Tip>
      <AgentStandingBadge v-else-if="agent.tone === 'needs' || agent.tone === 'failed'" :agent="agent" />
      <div class="presence" :class="doingText(agent.doing_tone) || 'text-text2'" role="status"><LiveDot :live="agent.doing_tone === 'busy'" /><Tip :text="agent.doing" when-clipped><span class="min-w-0 truncate">{{ agent.doing }}</span></Tip></div>
    </template>
    <template #end><slot name="actions" /></template>
    <template #tabs>
      <TabsTrigger v-if="decided" value="decisions" :aria-label="decisionsNew ? `Decisions, ${decisionsNew} need you` : undefined">Decisions<Badge v-if="decisionsNew" variant="needs" size="count" data-decisions-new>{{ decisionsNew }}</Badge></TabsTrigger>
      <TabsTrigger value="activity">Activity</TabsTrigger>
      <TabsTrigger value="plan">Plan</TabsTrigger>
      <TabsTrigger value="session">Session</TabsTrigger>
      <TabsTrigger v-if="agent.remembers" value="memory">Memory</TabsTrigger>
    </template>
    <template #aside>
      <ContextMeter :used="card.ctx_used" :size="card.ctx_size" @show="$emit('context')" />
    </template>
  </PageHead>
</template>

<style scoped>
/* What the agent is doing, in the colour its doing tone names (doingText), its dot in the same; the dot pings while it works. It
   takes what the row leaves and truncates in it. */
.presence {
  display: flex;
  flex: 1;
  align-items: center;
  gap: 6px;
  min-width: 0;
  font-size: 12px;
  white-space: nowrap;
}
</style>
