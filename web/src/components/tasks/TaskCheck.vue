<script setup lang="ts">
// The hollow circle in front of a task (Relay's approvals). Pressing it asks the Assistant to complete the task;
// a done task shows a filled check and asks to reopen it.
import { Check } from '@lucide/vue'
import type { DocSummary } from '../../core/generated/DocSummary'
import { ask, busy } from '../../assistant'

const props = defineProps<{ task: DocSummary; done?: boolean }>()
function toggle() {
  void ask(props.done ? `Reopen the task “${props.task.title}” (ID ${props.task.id}).` : `Complete the task “${props.task.title}” (ID ${props.task.id}).`)
}
</script>

<template>
  <button
    type="button"
    class="group/check relative z-10 grid size-[22px] shrink-0 place-items-center rounded-full border-[1.5px] transition-colors focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-ring disabled:opacity-40"
    :class="done ? 'border-ink bg-ink text-panel' : 'border-line2 text-transparent hover:border-ink2 hover:text-ink2'"
    :disabled="busy()"
    :aria-label="done ? `Ask the Assistant to reopen: ${task.title}` : `Ask the Assistant to complete: ${task.title}`"
    :title="done ? 'Done · ask to reopen' : 'Ask the Assistant to complete'"
    @click.stop="toggle"
  >
    <Check class="size-3" stroke-width="3" aria-hidden="true" />
  </button>
</template>
