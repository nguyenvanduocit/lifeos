<script setup lang="ts">
import { ref } from 'vue';
import { copyText } from './clipboard';
import Icon from './Icon.vue';
import { Button } from './ui/button';

// Copy for the text shown beside it. `label` names the button for a screen reader, as the parent's words say what is
// copied. `iconOnly` where a word would crowd what it sits on (a message in the feed). The button says for a moment
// whether the text went to the clipboard (remit says it in a toast; the panel has none).
defineProps<{ text: string; label: string; iconOnly?: boolean }>();
const said = ref<'' | 'Copied' | 'Could not copy'>('');
let timer: ReturnType<typeof setTimeout> | undefined;
async function copy(text: string) {
  said.value = (await copyText(text)) ? 'Copied' : 'Could not copy';
  clearTimeout(timer);
  timer = setTimeout(() => (said.value = ''), 1500);
}
</script>

<template>
  <Button variant="outline" :size="iconOnly ? 'icon-xs' : 'xs'" class="flex-none" :aria-label="said || label" @click="copy(text)"><Icon :name="said === 'Copied' ? 'check' : 'copy'" :size="12" /><template v-if="!iconOnly"> {{ said || 'Copy' }}</template></Button>
</template>
