<script setup lang="ts">
// The crew as Relay lists agents: one row per member (who it is, what it last said, its model and effort, how much it
// remembers, whether it is working), each opening its own page. Nothing is edited here: the owner tells the Assistant,
// and the list changes as it saves.
import { computed } from 'vue'
import {
  Archive, BookOpen, Bot, Brain, Cpu, Gauge, HeartPulse, Lightbulb, MessageCircle, Plus, RotateCcw, UserPlus, Wallet, Activity,
} from '@lucide/vue'
import { useRead } from '../data'
import { ago } from '../components/agents/when'
import { ASSISTANT, agentState, busy, panel, panelOpen, talkTo } from '../assistant'
import AgentBadge from '../components/chat/AgentBadge.vue'
import AskChip from '../components/AskChip.vue'
import LoadState from '../components/LoadState.vue'
import PageHeader from '../components/ds/PageHeader.vue'
import StatStrip from '../components/ds/StatStrip.vue'
import Panel from '../components/ds/Panel.vue'
import TableHead from '../components/ds/TableHead.vue'
import StatusTag from '../components/ds/StatusTag.vue'
import IconTile from '../components/ds/IconTile.vue'

const { data, error, loading, retry } = useRead('list_crew')
const active = computed(() => data.value?.filter((m) => m.status === 'active') ?? [])
const retired = computed(() => data.value?.filter((m) => m.status !== 'active') ?? [])
const working = computed(() => active.value.filter((m) => busy(m.id)).length)
const memories = computed(() => active.value.reduce((n, m) => n + m.memories, 0))

// The final message of its last finished turn, as its card keeps it.
function lastWords(id: string): string {
  const out = (agentState(id).view?.card.output ?? '').replace(/[#*_`>]/g, '').replace(/\s+/g, ' ').trim()
  return out.length > 140 ? out.slice(0, 139) + '…' : out
}

const strip = computed(() => [
  { key: 'active', icon: Bot, value: String(active.value.length), label: retired.value.length ? `active · ${retired.value.length} retired` : 'active agents' },
  { key: 'working', icon: Activity, value: String(working.value), label: 'working now', urgent: false },
  { key: 'mem', icon: Brain, value: String(memories.value), label: 'memories kept', to: '/memory' },
])

const GRID = 'minmax(0,2.2fr) minmax(0,1fr) minmax(0,0.7fr) minmax(0,0.7fr) minmax(0,0.8fr) 6.5rem'
const GRID_SM = 'minmax(0,1fr) 6.5rem'
const cols = [
  { label: 'Agent', icon: Bot },
  { label: 'Model', icon: Cpu, class: 'hidden @min-[44rem]:flex' },
  { label: 'Effort', icon: Gauge, class: 'hidden @min-[44rem]:flex' },
  { label: 'Memory', icon: Brain, class: 'hidden @min-[44rem]:flex' },
  { label: 'Status', icon: Activity, class: 'hidden @min-[44rem]:flex' },
  { label: '' },
]

const STARTERS = [
  { label: 'Add a health coach', icon: HeartPulse, words: 'Create a health coach to track my sleep, weight, and runs.' },
  { label: 'Add a money keeper', icon: Wallet, words: 'I want a dedicated agent to categorize my spending and warn me before a budget breaks.' },
  { label: 'Add a reading companion', icon: BookOpen, words: 'I want a dedicated agent to keep my reading notes and quiz me on what I read.' },
]
</script>

<template>
  <PageHeader title="Crew" :eyebrow="data ? `${active.length} agent${active.length === 1 ? '' : 's'} · led by the Assistant` : undefined">
    <AskChip size="sm" :icon="Lightbulb" words="Suggest agents based on my current goals and habits." label="Suggest agents" />
    <AskChip size="sm" primary :icon="UserPlus" words="I want a dedicated agent to " label="Add an agent" />
    <template v-if="active.length" #strip><StatStrip :items="strip" /></template>
  </PageHeader>

  <LoadState :loading="loading && !data" :error="error" label="crew" class="mb-4" @retry="retry" />
  <template v-if="data">
    <Panel title="Agents" :icon="Bot" :count="active.length">
      <template #actions>
        <button type="button" class="inline-flex h-8 items-center gap-1.5 rounded-[10px] px-2.5 text-[13px] text-ink2 hover:bg-panel2 hover:text-ink" @click="talkTo(ASSISTANT)">
          <AgentBadge :id="ASSISTANT" name="Assistant" :size="16" /> Talk to the Assistant
        </button>
      </template>

      <div v-if="active.length" class="@container" role="table" aria-label="Crew">
        <TableHead :grid="GRID" :grid-sm="GRID_SM" :cols="cols" />
        <div v-for="m in active" :key="m.id" role="row" data-cols
          :style="{ '--cols': GRID, '--cols-sm': GRID_SM }"
          class="relative grid min-h-[68px] items-center gap-x-4 border-b border-line px-5 py-3 transition-colors last:border-b-0 hover:bg-panel2/60"
          :class="panelOpen && panel.agent === m.id ? 'bg-panel2/60' : ''">
          <div role="cell" class="flex min-w-0 items-center gap-3">
            <AgentBadge :id="m.id" :name="m.name" :size="36" />
            <div class="min-w-0">
              <RouterLink :to="`/crew/${m.id}`" class="block truncate text-[14.5px] font-medium after:absolute after:inset-0 focus-visible:outline-none focus-visible:after:outline-2 focus-visible:after:outline-ring">{{ m.name }}</RouterLink>
              <p class="truncate text-[13px] text-ink2">{{ m.role ?? 'Crew member' }}</p>
              <p v-if="lastWords(m.id)" class="mt-0.5 truncate text-[12.5px] text-ink3" :title="lastWords(m.id)">“{{ lastWords(m.id) }}”</p>
            </div>
          </div>
          <div role="cell" class="hidden min-w-0 truncate text-[13.5px] text-ink2 @min-[44rem]:block">{{ m.model ?? 'Default' }}</div>
          <div role="cell" class="hidden text-[13.5px] text-ink2 capitalize @min-[44rem]:block">{{ m.effort ?? 'Default' }}</div>
          <div role="cell" class="hidden text-[13.5px] tabular-nums @min-[44rem]:block" :class="m.memories ? '' : 'text-ink3'">{{ m.memories || '—' }}</div>
          <div role="cell" class="hidden @min-[44rem]:block">
            <StatusTag v-if="busy(m.id)" tone="good" pulse>Working</StatusTag>
            <StatusTag v-else tone="neutral">Idle</StatusTag>
            <p v-if="agentState(m.id).view?.card.updated" class="mt-1 text-[12px] text-ink3" :title="agentState(m.id).view?.card.updated">{{ ago(agentState(m.id).view!.card.updated) }}</p>
          </div>
          <div role="cell" class="relative z-10 flex justify-end">
            <button type="button" class="inline-flex h-8 items-center gap-1.5 rounded-[10px] border border-line2 bg-panel px-3 text-[13px] font-medium hover:bg-panel2"
              :aria-label="`Chat with ${m.name}`" @click="talkTo(m.id)">
              <MessageCircle class="size-3.5" aria-hidden="true" /> Chat
            </button>
          </div>
        </div>
      </div>

      <div class="flex flex-wrap items-center gap-3 border-t border-line px-5 py-4" :class="active.length ? '' : 'py-8'">
        <IconTile :icon="Plus" tone="crew" />
        <div class="min-w-0 flex-1">
          <p class="text-[14.5px] font-medium">{{ active.length ? 'Grow your crew' : 'No crew members yet' }}</p>
          <p class="text-[13px] text-ink2">A crew member is an agent with one job: its own role, instructions, memory and tools. The Assistant sends it work.</p>
        </div>
        <div class="flex flex-wrap gap-2">
          <AskChip v-for="s in STARTERS" :key="s.label" :icon="s.icon" :label="s.label.replace('Add a ', '')" :words="s.words" />
        </div>
      </div>
    </Panel>

    <Panel v-if="retired.length" title="Retired" :icon="Archive" :count="retired.length" class="mt-5">
      <ul>
        <li v-for="m in retired" :key="m.id" class="relative flex items-center gap-3 border-t border-line px-5 py-3">
          <span class="opacity-60 grayscale"><AgentBadge :id="m.id" :name="m.name" :size="28" /></span>
          <div class="min-w-0 flex-1">
            <RouterLink :to="`/crew/${m.id}`" class="block truncate text-[14px] text-ink2 after:absolute after:inset-0">{{ m.name }}</RouterLink>
            <p class="truncate text-[12.5px] text-ink3">{{ m.role ?? 'Crew member' }}</p>
          </div>
          <AskChip class="relative z-10" :icon="RotateCcw" :words="`Reactivate ${m.name} (ID ${m.id}).`" label="Reactivate" />
        </li>
      </ul>
    </Panel>
  </template>
</template>
