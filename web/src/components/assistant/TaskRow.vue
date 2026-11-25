<script setup lang="ts">
import { computed, ref } from 'vue';
import { dataOf, stepLook } from './stream';
import { durSec } from './look';
import type { EventRead } from '../../core/generated/EventRead';
import CodeBlock from './CodeBlock.vue';
import CallCard from './CallCard.vue';

// One background task of an agent (a Monitor, a command run in the background), drawn as a tool call is (CallCard): the
// tool, what it does and how it stands, and opened, what it runs and what it ended with, as the server kept it.
const props = defineProps<{ event: EventRead }>();
const open = ref(false);
const ICON: Record<string, string> = { Bash: 'terminal', Monitor: 'eye' };
const data = computed(() => dataOf(props.event));
const look = computed(() => stepLook(props.event.event.status));
const running = computed(() => look.value.working);
const ms = computed(() => Date.parse(props.event.event.updated) - Date.parse(props.event.event.created));
</script>

<template>
  <CallCard v-model:open="open" class="my-1.5" :icon="ICON[data.tool ?? ''] ?? 'gear'" :name="data.tool || 'task'" :summary="event.event.title" :look="look" :time="running ? 'running…' : durSec(ms)" :failed="event.event.status === 'failed'" :data-work="event.event.id" data-task>
    <div class="flex flex-col gap-1.5" data-task-detail>
      <template v-if="data.command"><div class="caps">Command</div><CodeBlock class="max-h-[120px] overflow-auto" :text="data.command" label="Copy command" wrap /></template>
      <template v-if="event.event.text"><div class="caps">Output</div><CodeBlock class="max-h-[260px] overflow-auto" :text="event.event.text" label="Copy output" wrap /></template>
      <div v-if="!data.command && !event.event.text" class="text-faint">{{ running ? 'Nothing written yet.' : 'No details.' }}</div>
    </div>
  </CallCard>
</template>
