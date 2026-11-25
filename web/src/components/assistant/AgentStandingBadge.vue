<script setup lang="ts">
import { agentTone, badgeGlyph } from './look';
import type { AgentTone } from '../../core/generated/AgentTone';
import StandingBadge from './StandingBadge.vue';

// Where an agent stands, as the server says (its tone and label), alike in its row of an outline, its drawer, the Live tab
// and its seat at work. The ring turns while its time counts as work (`warms`: it runs, held up by nothing known), so an
// agent that runs but waits on a provider or a permission stands still. `lined`: its WorkLine beside it already pings
// while it warms, so the badge keeps its icon and the agent has one moving mark.
const props = defineProps<{ agent: { tone: AgentTone; label: string }; lined?: boolean }>();
</script>

<template>
  <StandingBadge :variant="agentTone(agent.tone)" :working="agent.tone === 'running' && !props.lined" :icon="badgeGlyph(agent.tone)" :data-tone="agent.tone">{{ agent.label }}</StandingBadge>
</template>
