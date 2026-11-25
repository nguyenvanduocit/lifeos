<script setup lang="ts">
import { fullTime, tokens } from './look';
import type { Card } from '../../core/generated/Card';
import KeyValues from './KeyValues.vue';

// How the agent runs: the model and effort LifeOS asks Claude Code for, the account it runs on, how many turns it took, how
// full its context is, its session and directory.
defineProps<{ card: Card }>();
</script>

<template>
  <KeyValues>
    <dt>Agent</dt><dd>{{ card.name }}</dd>
    <dt>Asked for</dt><dd>{{ card.model || 'default model' }} · effort {{ card.effort || 'not set' }}</dd>
    <dt>Account</dt><dd>{{ card.account || '—' }}</dd>
    <dt>Status</dt><dd>{{ card.status }}<template v-if="card.queued"> · {{ card.queued }} queued</template></dd>
    <dt>Turns</dt><dd>{{ card.turns }}</dd>
    <dt>Context</dt><dd>{{ card.ctx_size ? `${tokens(card.ctx_used)} of ${tokens(card.ctx_size)} tokens` : '—' }}</dd>
    <dt>Session</dt><dd class="mono">{{ card.session_id || '—' }}</dd>
    <dt>Directory</dt><dd class="mono">{{ card.cwd || '—' }}</dd>
    <dt>Created</dt><dd>{{ fullTime(card.created) }}</dd>
    <template v-if="card.error"><dt>Error</dt><dd><span class="text-red-text">{{ card.error }}</span></dd></template>
  </KeyValues>
</template>
