<script setup lang="ts">
// A suggestion for the agent. `say` puts the words in the Assistant's box to
// finish; `now` sends them at once (for unambiguous asks like "mark it done").
import { Sparkles } from '@lucide/vue'
import { ask, say } from '../assistant'

const props = defineProps<{ words: string; label?: string; now?: boolean }>()

function go() {
  if (props.now) void ask(props.words)
  else say(props.words)
}
</script>

<template>
  <button
    type="button"
    class="inline-flex items-center gap-1 rounded-full border border-line px-2 py-0.5 text-xs text-ink2 hover:border-accent hover:text-accent"
    :title="now ? 'Nhờ agent: ' + words : 'Viết cho agent: ' + words"
    @click="go"
  >
    <Sparkles :size="12" />
    {{ label ?? words }}
  </button>
</template>
