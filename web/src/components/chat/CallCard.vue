<script setup lang="ts">
// One tool call: its name and state; open it for the parameters and result.
import { computed } from 'vue'
import { Check, ChevronRight, Loader2, X } from '@lucide/vue'
import type { Call } from './blocks'
import { pretty } from './blocks'
import { toolWords } from '../../assistant'

const props = defineProps<{ item: Call }>()
const state = computed(() => (!props.item.outcome ? 'running' : props.item.outcome.failed ? 'failed' : 'done'))
const input = computed(() => pretty(props.item.call.input))
const output = computed(() => (props.item.outcome ? pretty(props.item.outcome.output) : ''))
</script>

<template>
  <details class="group rounded-lg border bg-panel text-xs" :class="state === 'failed' ? 'border-critical/50' : 'border-line'">
    <summary class="flex cursor-pointer list-none items-center gap-2 px-2.5 py-1.5">
      <ChevronRight :size="12" class="shrink-0 text-ink3 transition group-open:rotate-90" />
      <span class="font-mono text-[11.5px] text-ink">{{ item.call.name }}</span>
      <span class="truncate text-ink3">{{ toolWords(item.call.name) }}</span>
      <span class="ml-auto shrink-0">
        <Loader2 v-if="state === 'running'" :size="12" class="animate-spin text-ink3" />
        <X v-else-if="state === 'failed'" :size="13" class="text-critical" aria-label="Lỗi" />
        <Check v-else :size="13" class="text-good" aria-label="Xong" />
      </span>
    </summary>
    <div class="space-y-2 border-t border-line px-2.5 py-2">
      <div>
        <div class="mb-1 text-[10.5px] font-semibold uppercase tracking-wide text-ink3">Tham số</div>
        <pre class="max-h-[220px] overflow-auto rounded-md border border-line bg-bg p-2 font-mono text-[11px] whitespace-pre-wrap">{{ input }}</pre>
      </div>
      <div v-if="item.outcome">
        <div class="mb-1 text-[10.5px] font-semibold uppercase tracking-wide" :class="item.outcome.failed ? 'text-critical' : 'text-ink3'">
          {{ item.outcome.failed ? 'Lỗi' : 'Kết quả' }}
        </div>
        <pre class="max-h-[220px] overflow-auto rounded-md border border-line bg-bg p-2 font-mono text-[11px] whitespace-pre-wrap">{{ output }}</pre>
      </div>
    </div>
  </details>
</template>
