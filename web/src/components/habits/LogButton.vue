<script setup lang="ts">
// Log a habit for today: one outcome, so it asks the Assistant directly; once done it is a soft lime "Done".
import { Check } from '@lucide/vue'
import { ask, busy } from '../../assistant'

defineProps<{ habit: string; done: boolean; size?: 'sm' }>()
</script>

<template>
  <button type="button"
    class="relative z-10 inline-flex shrink-0 items-center border font-medium transition-colors disabled:cursor-default focus-visible:outline-2 focus-visible:outline-ring"
    :class="[size === 'sm' ? 'h-8 gap-1.5 rounded-lg px-2.5 text-[13px]' : 'h-8 gap-1.5 rounded-[10px] px-3 text-[13px]', done ? 'border-transparent bg-signal/40 text-signal-ink dark:bg-signal/15' : 'border-line2 bg-panel hover:bg-panel2']"
    :disabled="done || busy()"
    :aria-label="done ? `${habit}: done today` : `Ask the Assistant to log ${habit} for today`"
    @click="ask(`Log “${habit}” for today.`)">
    <Check class="size-3.5" aria-hidden="true" />{{ done ? 'Done' : 'Log today' }}
  </button>
</template>
