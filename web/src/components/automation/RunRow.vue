<script setup lang="ts">
// One run of a routine as a row: how it went, what set it off, when and how long, and the first words of the reply or
// the error. "Open chat" opens the agent's conversation in the panel.
import { MessageCircle, Zap } from '@lucide/vue'
import type { Run } from '../../core/generated/Run'
import { talkTo } from '../../assistant'
import StatusTag from '../ds/StatusTag.vue'
import { ago, plain, took } from '../agents/when'
import { TRIGGERS, runLook } from './looks'

defineProps<{ run: Run; agentName: string; grid: string; gridSm: string }>()
</script>

<template>
  <div role="row" data-cols :style="{ '--cols': grid, '--cols-sm': gridSm }"
    class="grid items-start gap-x-4 border-b border-line px-5 py-3 last:border-b-0">
    <div role="cell" class="min-w-0">
      <p class="truncate text-[14px] font-medium">{{ run.title }}</p>
      <p v-if="run.error" class="mt-1 rounded-lg bg-critical-wash px-2 py-1 text-[12.5px] break-words text-critical">{{ run.error }}</p>
      <p v-else-if="run.reply" class="mt-0.5 line-clamp-1 text-[13px] break-words text-ink2" :title="plain(run.reply)">{{ plain(run.reply) }}</p>
      <button type="button" class="mt-1 inline-flex items-center gap-1 text-[12.5px] text-ink3 hover:text-ink" @click="talkTo(run.agent)">
        <MessageCircle class="size-3" aria-hidden="true" /> {{ agentName }}’s chat
      </button>
    </div>
    <div role="cell" class="hidden min-w-0 items-center gap-1.5 text-[13px] text-ink2 @min-[44rem]:flex" :title="run.detail ?? undefined">
      <component :is="TRIGGERS[run.trigger]?.icon ?? Zap" class="size-3.5 shrink-0 text-ink3" aria-hidden="true" />
      <span class="truncate">{{ TRIGGERS[run.trigger]?.label ?? run.trigger }}<template v-if="run.detail"> · {{ run.detail }}</template></span>
    </div>
    <div role="cell" class="hidden text-[13px] text-ink2 tabular-nums @min-[44rem]:block">
      <time :datetime="run.queued.replace(' ', 'T')" :title="run.queued">{{ ago(run.queued) }}</time>
      <span v-if="took(run.ms)" class="block text-[12px] text-ink3">took {{ took(run.ms) }}</span>
    </div>
    <div role="cell" class="flex justify-end @min-[44rem]:justify-start">
      <StatusTag :tone="runLook(run).tone" :pulse="runLook(run).pulse">{{ runLook(run).label }}</StatusTag>
    </div>
  </div>
</template>
