<script setup lang="ts">
// A task as a line. Marking it done is asked of the agent, like every change.
import { computed } from 'vue'
import { Check, Circle, Flag } from '@lucide/vue'
import type { DocSummary } from '../core/generated/DocSummary'
import { ask, busy } from '../assistant'
import { relativeDay, todayIso } from '../format'

const props = defineProps<{ task: DocSummary; done?: boolean }>()
const when = computed(() => {
  const t = props.task
  if (t.due) return { text: 'hạn ' + relativeDay(t.due), late: t.due < todayIso() }
  if (t.scheduled) return { text: relativeDay(t.scheduled), late: false }
  return null
})

function complete() {
  void ask(`Đánh dấu xong việc “${props.task.title}” (id ${props.task.id}).`)
}
</script>

<template>
  <li class="group flex items-center gap-2 py-1 text-sm">
    <button
      type="button"
      class="text-ink3 hover:text-accent disabled:opacity-40"
      :disabled="done || busy()"
      :title="done ? 'Đã xong' : 'Nhờ agent đánh dấu xong'"
      @click="complete"
    >
      <Check v-if="done" :size="16" class="text-good" />
      <Circle v-else :size="16" />
    </button>
    <RouterLink :to="`/doc/${task.id}`" class="min-w-0 flex-1 truncate hover:underline" :class="done ? 'text-ink3 line-through' : ''">
      {{ task.title }}
    </RouterLink>
    <Flag v-if="task.priority === 'high'" :size="13" class="text-critical" aria-label="Ưu tiên cao" />
    <span v-if="when" class="shrink-0 text-xs" :class="when.late ? 'text-critical' : 'text-ink3'">{{ when.text }}</span>
  </li>
</template>
