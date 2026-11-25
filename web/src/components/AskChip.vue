<script setup lang="ts">
// A suggestion for the agent. `say` puts the words in the Assistant's box to
// finish; `now` sends them at once (for unambiguous asks like "mark it done").
import { Sparkles } from '@lucide/vue'
import { Button } from './ui/button'
import { ask, say } from '../assistant'

const props = defineProps<{ words: string; label?: string; now?: boolean; size?: 'xs' | 'sm'; primary?: boolean }>()

function go() {
  if (props.now) void ask(props.words)
  else say(props.words)
}
</script>

<template>
  <Button
    type="button"
    :variant="primary ? 'default' : 'outline'"
    :size="size ?? 'xs'"
    class="rounded-full"
    :class="{ 'text-ink2': !primary }"
    :title="now ? 'Nhờ agent: ' + words : 'Viết cho agent: ' + words"
    @click="go"
  >
    <Sparkles :size="12" />
    {{ label ?? words }}
  </Button>
</template>
