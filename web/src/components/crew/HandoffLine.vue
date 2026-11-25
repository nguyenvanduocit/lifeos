<script setup lang="ts">
// One hand-off as a row (an agent's page, the automation page): sender → target, the first words, when, how it stands.
// It opens in Team talk, in its thread.
import { ArrowRight } from '@lucide/vue'
import type { Handoff } from '../../core/generated/Handoff'
import AgentBadge from '../chat/AgentBadge.vue'
import StatusTag from '../ds/StatusTag.vue'
import { ago, plain } from '../agents/when'
import { handoffLook } from './handoff'

defineProps<{ handoff: Handoff }>()
</script>

<template>
  <li class="relative flex items-start gap-3 border-b border-line px-5 py-3 transition-colors last:border-b-0 hover:bg-panel2/60">
    <span class="mt-0.5 flex shrink-0 items-center gap-1">
      <AgentBadge :id="handoff.sender" :name="handoff.sender_name" :size="20" />
      <ArrowRight class="size-3 text-ink3" aria-hidden="true" />
      <AgentBadge :id="handoff.target" :name="handoff.target_name" :size="20" />
    </span>
    <div class="min-w-0 flex-1">
      <RouterLink :to="`/talk?h=${handoff.id}`" class="block truncate text-[14px] after:absolute after:inset-0">{{ plain(handoff.text) }}</RouterLink>
      <p class="mt-0.5 truncate text-[12.5px] text-ink3">
        {{ handoff.sender_name }} → {{ handoff.target_name }} · {{ ago(handoff.created) }}<template v-if="handoff.note"> · <span class="text-warning">{{ handoff.note }}</span></template>
      </p>
    </div>
    <StatusTag :tone="handoffLook(handoff).tone" :pulse="handoff.status === 'working'">{{ handoffLook(handoff).label }}</StatusTag>
  </li>
</template>
