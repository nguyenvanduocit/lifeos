<script setup lang="ts">
// One run of a workflow script as a row: its name, who started it, how it stands and how far its steps got; opened,
// each step (agents put to work, waits, questions to the owner) and what it returned or why it failed.
import { computed, ref } from 'vue'
import { ChevronDown, FileCode2 } from '@lucide/vue'
import type { WorkflowRun } from '../../core/generated/WorkflowRun'
import IconTile from '../ds/IconTile.vue'
import StatusTag from '../ds/StatusTag.vue'
import { ago } from '../agents/when'
import { WORKFLOW_STATUS } from './looks'

const props = defineProps<{ run: WorkflowRun }>()
const open = ref(false)
const look = computed(() => WORKFLOW_STATUS[props.run.status] ?? { label: props.run.status, tone: 'neutral' })
const done = computed(() => props.run.steps.filter((s) => s.status === 'done').length)
const STEP_TONE: Record<string, string> = { done: 'good', failed: 'critical', pending: 'info' }
</script>

<template>
  <li class="border-b border-line last:border-b-0">
    <button type="button" class="flex w-full min-w-0 items-center gap-3.5 px-5 py-3.5 text-left transition-colors hover:bg-panel2/60" :aria-expanded="open" @click="open = !open">
      <IconTile :icon="FileCode2" tone="automation" size="sm" />
      <span class="min-w-0 flex-1">
        <span class="block truncate text-[14.5px] font-medium">{{ run.name }}</span>
        <span class="block truncate text-[12.5px] text-ink3">{{ run.description || 'Workflow' }} · started by {{ run.started_by }} · {{ ago(run.created) }}</span>
      </span>
      <span class="hidden w-28 shrink-0 text-[13px] text-ink2 tabular-nums @min-[44rem]:block">{{ done }} / {{ run.steps.length }} steps</span>
      <StatusTag :tone="look.tone" :pulse="look.pulse">{{ look.label }}</StatusTag>
      <ChevronDown class="size-4 shrink-0 text-ink3 transition-transform" :class="open ? 'rotate-180' : ''" aria-hidden="true" />
    </button>
    <div v-if="open && (run.error || run.steps.length || run.result)" class="space-y-3 px-5 pb-4 pl-[3.85rem] text-[13px]">
      <p v-if="run.error" class="rounded-lg bg-critical-wash px-3 py-2 break-words text-critical">{{ run.error }}</p>
      <ol v-if="run.steps.length" class="overflow-hidden rounded-xl border border-line">
        <li v-for="s in run.steps" :key="s.seq" class="flex items-center gap-3 border-b border-line px-3.5 py-2 last:border-b-0">
          <span class="w-6 shrink-0 text-right text-[12px] text-ink3 tabular-nums">{{ s.seq }}</span>
          <code class="shrink-0 rounded-md bg-panel2 px-1.5 py-0.5 font-mono text-[11.5px] text-ink2">{{ s.verb }}</code>
          <span class="min-w-0 flex-1 truncate" :title="s.error || s.label">{{ s.label || s.phase }}</span>
          <StatusTag :tone="STEP_TONE[s.status] ?? 'neutral'">{{ s.status }}</StatusTag>
        </li>
      </ol>
      <div v-if="run.result">
        <p class="mb-1 text-[12.5px] font-medium text-ink3">Returned</p>
        <pre class="max-h-56 overflow-auto rounded-xl border border-line bg-panel2 px-3.5 py-2.5 font-mono text-[12px] whitespace-pre-wrap text-ink2">{{ run.result }}</pre>
      </div>
    </div>
  </li>
</template>
