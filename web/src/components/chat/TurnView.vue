<script setup lang="ts">
// One exchange: your message as a bubble, then what the agent did. A
// finished turn folds its steps behind one line ("Đã làm 12 giây · 3 tool")
// and keeps its words in view; failures stay visible. A live turn shows
// everything as it happens.
import { computed, ref } from 'vue'
import { ChevronRight, TriangleAlert } from '@lucide/vue'
import type { Step } from '../../core/generated/Step'
import Markdown from '../Markdown.vue'
import AgentBadge from './AgentBadge.vue'
import ThinkingFold from './ThinkingFold.vue'
import ToolGroup from './ToolGroup.vue'
import { blocks, duration, failed, type Call } from './blocks'

const props = defineProps<{
  ask: string
  at?: string
  steps: Step[]
  agent: { id: string; name: string }
  done?: { ms: number; error: string | null }
  live?: boolean
  startedAt?: number
}>()

const all = computed(() => blocks(props.steps))
const calls = computed((): Call[] => all.value.flatMap((b) => (b.kind === 'tools' ? b.calls : [])))
const failures = computed(() => failed(calls.value))
const hasSteps = computed(() => all.value.some((b) => b.kind !== 'text'))
const open = ref(false)
// Folded, a finished turn keeps its words and any tool group that failed.
const shown = computed(() =>
  props.live || open.value
    ? all.value
    : all.value.filter((b) => b.kind === 'text' || (b.kind === 'tools' && failed(b.calls) > 0)),
)
const lastIndex = computed(() => all.value.length - 1)
const time = computed(() => props.at?.slice(11) ?? '')
const working = computed(() => {
  const last = all.value[lastIndex.value]
  return props.live && !(last?.kind === 'tools' && last.calls.some((c) => !c.outcome))
})
</script>

<template>
  <div class="space-y-3">
    <div class="flex flex-col items-end gap-1 pl-10">
      <span class="text-[10.5px] text-ink3">Bạn<template v-if="time"> · {{ time }}</template></span>
      <div class="owner-bubble px-3 py-2 text-sm leading-relaxed whitespace-pre-wrap">{{ ask }}</div>
    </div>

    <div class="flex gap-2.5">
      <AgentBadge :id="agent.id" :name="agent.name" class="mt-0.5" />
      <div class="min-w-0 flex-1 space-y-2">
        <button
          v-if="!live && hasSteps"
          type="button"
          class="flex items-center gap-1.5 text-xs text-ink3 hover:text-ink"
          @click="open = !open"
        >
          <ChevronRight :size="12" class="transition" :class="open ? 'rotate-90' : ''" />
          Đã làm trong {{ duration(done?.ms ?? 0) }}<template v-if="calls.length"> · {{ calls.length }} tool</template>
          <span v-if="failures" class="text-critical">· {{ failures }} lỗi</span>
        </button>

        <template v-for="(b, i) in shown" :key="i">
          <Markdown v-if="b.kind === 'text'" :source="b.text" class="text-sm text-ink" />
          <ThinkingFold v-else-if="b.kind === 'thinking'" :text="b.text" :live="live && i === lastIndex" />
          <ToolGroup v-else :calls="b.calls" :live="live && i === lastIndex" />
        </template>

        <div v-if="working" class="sheen text-xs font-medium">{{ agent.name }} đang làm…</div>

        <div v-if="done?.error" class="flex gap-2 rounded-lg border border-critical/40 bg-critical-wash px-3 py-2 text-xs text-critical">
          <TriangleAlert :size="14" class="mt-px shrink-0" />
          <span class="whitespace-pre-wrap">{{ done.error }}</span>
        </div>
      </div>
    </div>
  </div>
</template>
