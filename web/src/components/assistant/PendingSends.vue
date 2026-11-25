<script setup lang="ts">
import { resend, unsend, type Pending } from '../../assistant';
import { fileName, isFile } from '../../images';
import { Button } from './ui/button';
import Icon from './Icon.vue';
import Spinner from './Spinner.vue';

// The owner's messages to agent `card` the server has not taken yet: the words as sent, and under them what is happening to
// them. A send asked a moment ago says nothing; one still unanswered after a moment says it is sending; one that failed
// says why, with its one way out: try it again, or take the words back into the box to edit them.
defineProps<{ card: string; pending: Pending[] }>();
function label(p: Pending) {
  if (p.text) return p.text;
  if (p.images.length === 1) {
    const item = p.images[0];
    return isFile(item) ? fileName(item) : 'An image';
  }
  return `${p.images.length} attachments`;
}
</script>

<template>
  <ul v-if="pending.length" class="flex flex-col items-end gap-1.5 pb-2" aria-label="Messages on their way">
    <li v-for="p in pending" :key="p.id" class="flex max-w-[85%] flex-col items-end gap-1" :data-pending="p.status">
      <p class="bubble whitespace-pre-wrap px-3 py-2 text-[13px] leading-relaxed" :class="p.status === 'failed' ? 'border-red-line' : 'border-line2'">{{ label(p) }}</p>
      <p v-if="p.status === 'slow'" class="flex items-center gap-1.5 text-[11.5px] text-mut" role="status"><Spinner :size="10" />Sending…</p>
      <div v-else-if="p.status === 'failed'" class="flex flex-wrap items-center justify-end gap-1.5 text-[11.5px] text-red-text" role="alert">
        <Icon name="alert" :size="12" /><span>Not sent: {{ p.why }}</span>
        <Button variant="outline" size="xs" data-resend @click="resend(card, p.id)"><Icon name="repeat" :size="11" />Try again</Button>
        <Button variant="ghost" size="xs" data-unsend @click="unsend(card, p.id)"><Icon name="pencil" :size="11" />Edit</Button>
      </div>
    </li>
  </ul>
</template>

<style scoped>
.bubble {
  border-width: 1px;
  border-radius: 12px;
  background: var(--panel2);
  color: var(--text);
}
</style>
