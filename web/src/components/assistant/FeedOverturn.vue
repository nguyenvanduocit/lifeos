<script setup lang="ts">
import { nextTick, ref, useId } from 'vue';
import { messageOf } from '../../assistant';
import { overturn } from '../../core/link';
import type { Question } from '../../core/generated/Question';
import { Button } from './ui/button';
import { Input } from './ui/input';

// Overturning one decision made for the owner (an answer the Assistant gave for him): a button that opens one box for his
// word, sent as overturn; `ask` is what the box asks him (what he decides instead, unless said otherwise) and `verb` what
// the button says. A question left to him is answered the same way. Where the decision shows, the overturn shows once the
// server pushes it; what did not go stays in the box, and he is told why.
const props = withDefaults(defineProps<{ question: Question; ask?: string; verb?: string }>(), { ask: 'What do you decide instead?', verb: 'Overturn' });
const opened = ref(false);
const word = ref('');
const sending = ref(false);
const failed = ref('');
const box = ref<InstanceType<typeof Input> | null>(null);
const id = useId();
async function open() {
  opened.value = true;
  failed.value = '';
  await nextTick();
  (box.value?.$el as HTMLInputElement | undefined)?.focus();
}
async function submit() {
  const text = word.value.trim();
  if (!text || sending.value) return;
  sending.value = true;
  failed.value = '';
  try {
    await overturn(props.question.id, text);
    word.value = '';
    opened.value = false;
  } catch (e) {
    failed.value = `Could not ${props.verb.toLowerCase()}: ${messageOf(e)}`;
  } finally {
    sending.value = false;
  }
}
</script>

<template>
  <form v-if="opened" class="mt-2 flex flex-col gap-1.5" data-overturn-form @submit.prevent="submit" @keydown.esc.stop="opened = false">
    <label :for="id" class="text-[11.5px] text-text2">{{ ask }}</label>
    <div class="flex gap-1.5">
      <Input :id="id" ref="box" v-model="word" size="sm" class="flex-1" placeholder="In a few words" data-overturn-word />
      <Button type="submit" size="sm" :disabled="sending || !word.trim()" data-overturn-send>{{ verb }}</Button>
      <Button type="button" variant="ghost" size="sm" @click="opened = false">Cancel</Button>
    </div>
    <p v-if="failed" class="m-0 text-[11.5px] text-red-text" role="alert">{{ failed }}</p>
  </form>
  <Button v-else variant="outline" size="xs" class="mt-1.5" :aria-label="`${verb}: ${question.text}`" data-overturn-open @click="open">{{ verb }}</Button>
</template>
