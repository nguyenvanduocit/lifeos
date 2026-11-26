<script setup lang="ts">
// One routine as a row: its mark, title, who does it and what it does, how it starts, when it runs next and how its
// last run went; opened, the webhook's how-to and what the Assistant can change. The chips only draft.
import { ref } from 'vue'
import { ChevronDown, FileCode2, History, Pause, Pencil, Play, Webhook } from '@lucide/vue'
import type { Routine } from '../../core/generated/Routine'
import AgentBadge from '../chat/AgentBadge.vue'
import AskChip from '../AskChip.vue'
import IconTile from '../ds/IconTile.vue'
import StatusTag from '../ds/StatusTag.vue'
import { ago, plain } from '../agents/when'
import { lead, routineLook, runLook, triggers } from './looks'

defineProps<{ routine: Routine; agentName: string }>()
defineEmits<{ runs: [id: string] }>()
const open = ref(false)
</script>

<template>
  <li class="border-b border-line last:border-b-0" :class="routine.status !== 'active' ? 'text-ink2' : ''">
    <div class="flex min-w-0 items-start gap-3.5 px-5 py-4">
      <IconTile :icon="lead(routine)" :tone="routine.status === 'active' ? 'automation' : 'neutral'" class="mt-0.5" />
      <div class="min-w-0 flex-1">
        <div class="flex flex-wrap items-center gap-x-2.5 gap-y-1">
          <h3 class="text-[15px] leading-snug font-medium wrap-anywhere text-ink">{{ routine.title }}</h3>
          <StatusTag :tone="routineLook(routine).tone" :pulse="routineLook(routine).pulse">{{ routineLook(routine).label }}</StatusTag>
        </div>
        <p class="mt-0.5 line-clamp-1 text-[13.5px] wrap-anywhere text-ink2" :title="routine.instructions">{{ plain(routine.instructions) }}</p>
        <div class="mt-2 flex flex-wrap items-center gap-1.5 text-[12.5px]">
          <span class="inline-flex h-6 items-center gap-1.5 rounded-md border border-line bg-panel2 px-2 text-ink2">
            <AgentBadge :id="routine.agent" :name="agentName" :size="14" /> {{ agentName }}
          </span>
          <span v-if="routine.workflow" class="inline-flex h-6 items-center gap-1.5 rounded-md border border-line bg-panel2 px-2 text-ink2">
            <FileCode2 class="size-3.5 text-ink3" aria-hidden="true" /> {{ routine.workflow }}
          </span>
          <span v-for="t in triggers(routine)" :key="t.text" :title="t.title" class="inline-flex h-6 items-center gap-1.5 rounded-md border border-line bg-panel2 px-2 text-ink">
            <component :is="t.icon" class="size-3.5 text-ink3" aria-hidden="true" />
            <code v-if="t.code" class="font-mono text-[11.5px]">{{ t.text }}</code>
            <template v-else>{{ t.text }}</template>
          </span>
        </div>
      </div>
      <div class="hidden w-40 shrink-0 text-right text-[13px] @min-[44rem]:block">
        <p v-if="routine.next && routine.status === 'active'" :title="routine.next"><span class="text-ink3">Next</span> {{ ago(routine.next) }}</p>
        <p v-if="routine.last" class="mt-0.5 text-ink2" :title="routine.last.queued">
          <span :data-tone="runLook(routine.last).tone" class="text-tone-ink">●</span> {{ runLook(routine.last).label }} {{ ago(routine.last.queued) }}
        </p>
        <p v-else class="mt-0.5 text-ink3">Never ran</p>
        <p v-if="routine.runs > 1" class="mt-0.5 text-ink3 tabular-nums">{{ routine.runs }} runs</p>
      </div>
      <button type="button" class="grid size-8 shrink-0 place-items-center rounded-lg text-ink3 hover:bg-panel2 hover:text-ink" :aria-expanded="open" :aria-label="`More for ${routine.title}`" @click="open = !open">
        <ChevronDown class="size-4 transition-transform" :class="open ? 'rotate-180' : ''" />
      </button>
    </div>
    <div v-if="open" class="space-y-3 px-5 pb-4 pl-[4.1rem]">
      <p class="text-[13.5px] whitespace-pre-wrap wrap-anywhere text-ink2">{{ plain(routine.instructions) }}</p>
      <div v-if="routine.hook" class="rounded-xl border border-line bg-panel2 px-3.5 py-3 text-[12.5px] text-ink2">
        <p class="mb-1 flex items-center gap-1.5 font-medium text-ink"><Webhook class="size-3.5" aria-hidden="true" /> Webhook · {{ routine.hook_ready ? 'key configured' : 'key required' }}</p>
        <p>Create or rotate the key in a terminal on the LifeOS host: <code class="font-mono">lifeos hook-key {{ routine.id }}</code></p>
        <p class="mt-1 break-all">Send <code class="font-mono">POST /hooks/{{ encodeURIComponent(routine.id) }}</code> with <code class="font-mono">Authorization: Bearer &lt;key&gt;</code>.</p>
      </div>
      <div class="flex flex-wrap items-center gap-2">
        <AskChip v-if="routine.status !== 'paused'" :icon="Play" label="Run now" :words="`Run the routine with ID routine_${routine.id}.`" />
        <AskChip :icon="Pencil" label="Edit" :words="`Update routine routine_${routine.id} as follows: `" />
        <AskChip :icon="routine.status === 'paused' ? Play : Pause" :label="routine.status === 'paused' ? 'Resume' : 'Pause'"
          :words="`${routine.status === 'paused' ? 'Resume' : 'Pause'} routine routine_${routine.id}.`" />
        <button type="button" class="inline-flex h-8 items-center gap-1.5 rounded-[10px] px-2.5 text-[13px] text-ink2 hover:bg-panel2 hover:text-ink" @click="$emit('runs', routine.id)">
          <History class="size-3.5" aria-hidden="true" /> View runs
        </button>
      </div>
    </div>
  </li>
</template>
