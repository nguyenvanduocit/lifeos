<script setup lang="ts">
import { computed } from 'vue';
import { interrupt } from '../../assistant';
import { useStream } from './streamContext';
import { receipt } from './stream';
import { toneText } from './look';
import type { EventRead } from '../../core/generated/EventRead';
import Tip from './Tip.vue';
import { Button } from './ui/button';

// The line under a message put to an agent: whether it has arrived, is being read, or was read, from the message's status
// and the message the server says the agent is on, in the colour of its tone. While it waits for the agent's current work,
// the owner may end that turn so it is read now, where the server lets him.
const props = defineProps<{ event: EventRead }>();
const stream = useStream();
const standing = computed(() => stream.standingOf(props.event.event.card));
const r = computed(() => receipt(props.event, standing.value?.on_message));
</script>

<template>
  <div class="flex items-center gap-2 text-[10.5px]" data-receipt>
    <span :class="toneText[r.tone]">{{ r.text }}</span>
    <Tip v-if="r.queued && standing?.asks.includes('interrupt')" text="Interrupt the running turn and read this message now"><Button variant="outline" size="xs" @click="interrupt(event.event.card)">Send now</Button></Tip>
  </div>
</template>
