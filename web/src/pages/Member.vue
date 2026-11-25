<script setup lang="ts">
import EntityId from '../components/ds/EntityId.vue'
import PageActions from '../components/ds/PageActions.vue'
// One crew member's page: who it is and how it stands, its instructions, the work it handed and was handed, what it
// remembers and the routines it runs, with its settings in a Details card at the side. Changes are asked of the
// Assistant (the chips); "Chat" opens the member's own conversation in the panel.
import { computed } from 'vue'
import {
  Archive, Brain, CalendarClock, ChevronRight, Cpu, FileText, Gauge, MessageCircle, MessagesSquare, Pencil,
  Plug, RotateCcw, Wrench,
} from '@lucide/vue'
import { useRead } from '../data'
import { ASSISTANT, agentState, busy, talkTo } from '../assistant'
import AgentBadge from '../components/chat/AgentBadge.vue'
import AskChip from '../components/AskChip.vue'
import LoadState from '../components/LoadState.vue'
import Markdown from '../components/Markdown.vue'
import Panel from '../components/ds/Panel.vue'
import PropertyList from '../components/ds/PropertyList.vue'
import StatusTag from '../components/ds/StatusTag.vue'
import MemoryRows from '../components/crew/MemoryRows.vue'
import HandoffLine from '../components/crew/HandoffLine.vue'
import { ago } from '../components/agents/when'
import { routineLook, triggers } from '../components/automation/looks'

const props = defineProps<{ id: string }>()

const { data: m, error, loading, retry } = useRead('crew_member', () => ({ id: props.id }))
const { data: crew } = useRead('list_crew')
const { data: handoffs } = useRead('list_handoffs', () => ({ open: false, agent: props.id }))
const { data: memories } = useRead('list_memories', () => ({ holder: props.id, include_forgotten: false }))
const { data: routines } = useRead('list_routines')

const names = computed<Record<string, string>>(() => Object.fromEntries([[ASSISTANT, 'Assistant'], ...(crew.value ?? []).map((c) => [c.id, c.name])]))
const card = computed(() => agentState(props.id).view?.card)
const recent = computed(() => (handoffs.value ?? []).slice(0, 8))
const mine = computed(() => (routines.value ?? []).filter((r) => r.agent === props.id))
const retired = computed(() => m.value?.status !== 'active')

const rows = computed(() => {
  const x = m.value
  if (!x) return []
  return [
    { key: 'model', label: 'Model', icon: Cpu, value: x.model ?? 'Default' },
    { key: 'effort', label: 'Effort', icon: Gauge, value: x.effort ?? 'Default' },
    { key: 'tools', label: 'Tools', icon: Wrench, value: x.tools.length ? `${x.tools.length} allowed` : 'All LifeOS tools' },
    { key: 'connections', label: 'Connections', icon: Plug, value: x.connections.length ? x.connections.join(', ') : 'None' },
    { key: 'memories', label: 'Memories', icon: Brain, value: String(x.memories) },
    { key: 'id', label: 'File', icon: FileText, value: `crew/${x.id}.md` },
  ]
})
const ref_ = computed(() => (m.value ? `crew_${m.value.id}` : ''))
</script>

<template>
  <nav aria-label="Breadcrumb" class="mb-4 flex items-center gap-1.5 text-[14px] text-ink2">
    <RouterLink to="/crew" class="hover:text-ink">Crew</RouterLink>
    <ChevronRight class="size-3.5 text-ink3" aria-hidden="true" />
    <span class="truncate text-ink">{{ m?.name ?? id }}</span>
  </nav>

  <LoadState :loading="loading && !m" :error="error" label="crew member" class="mb-4" @retry="retry" />
  <div v-if="m" class="@container max-w-[1180px]">
    <header class="mb-7 flex flex-wrap items-center gap-x-5 gap-y-4">
      <span :class="retired ? 'opacity-60 grayscale' : ''"><AgentBadge :id="m.id" :name="m.name" :size="56" /></span>
      <div class="min-w-0 flex-1">
        <h1 class="flex flex-wrap items-center gap-3 text-[clamp(1.75rem,3.2vw,2.4rem)] leading-[1.1] font-semibold tracking-[-0.03em]">
          {{ m.name }}
          <StatusTag v-if="retired" tone="neutral">Retired</StatusTag>
          <StatusTag v-else-if="busy(m.id)" tone="good" pulse>Working</StatusTag>
          <StatusTag v-else tone="neutral">Idle</StatusTag>
        </h1>
        <EntityId :key="ref_" :value="ref_" />
        <p class="mt-1 text-[14.5px] text-ink2">{{ m.role ?? 'Crew member' }}<template v-if="card?.updated"> · active {{ ago(card.updated) }}</template></p>
      </div>
      <PageActions>
        <AskChip v-if="retired" size="sm" :icon="RotateCcw" :words="`Reactivate ${ref_}.`" label="Reactivate" />
        <template v-else>
          <AskChip size="sm" :icon="Archive" :words="`Retire ${ref_} because `" label="Retire" />
          <AskChip size="sm" :icon="Pencil" :words="`Update the instructions for ${ref_}: `" label="Edit instructions" />
          <button type="button" class="inline-flex h-8 items-center gap-1.5 rounded-lg bg-ink px-2.5 text-[13px] font-medium text-panel hover:bg-ink/90" @click="talkTo(m.id)">
            <MessageCircle class="size-3.5" aria-hidden="true" /> Chat with {{ m.name }}
          </button>
        </template>
      </PageActions>
    </header>

    <div class="grid items-start gap-6 @min-[56rem]:grid-cols-[minmax(0,1fr)_18rem]">
      <div class="order-2 rounded-2xl border border-line bg-panel2 px-4 py-4 @min-[56rem]:sticky @min-[56rem]:top-0">
        <p class="mb-2 text-[12.5px] font-medium text-ink3">Details</p>
        <PropertyList :rows="rows" stack>
          <template #tools>
            <span v-if="!m.tools.length">All LifeOS tools</span>
            <span v-else class="flex flex-wrap gap-1">
              <code v-for="t in m.tools" :key="t" class="rounded-md bg-panel px-1.5 py-0.5 font-mono text-[11.5px] text-ink2">{{ t }}</code>
            </span>
          </template>
          <template #memories>
            <RouterLink :to="{ path: '/memory', query: { agent: m.id } }" class="tabular-nums hover:underline">{{ m.memories }} kept</RouterLink>
          </template>
          <template #id><span class="truncate font-mono text-[12px] text-ink3">crew/{{ m.id }}.md</span></template>
        </PropertyList>
      </div>

      <div class="order-1 min-w-0 space-y-6">
        <Panel title="Instructions" :icon="FileText" pad>
          <template #actions>
            <AskChip :icon="Pencil" :words="`Update the instructions for ${ref_}: `" label="Edit" />
          </template>
          <Markdown v-if="m.instructions.trim()" :source="m.instructions" class="text-[14.5px] leading-relaxed" />
          <p v-else class="text-[14px] text-ink3">No instructions yet.</p>
        </Panel>

        <Panel title="Hand-offs" :icon="MessagesSquare" :count="handoffs?.length || undefined" :to="`/talk?agent=${m.id}`">
          <ul v-if="recent.length" class="border-t border-line">
            <HandoffLine v-for="h in recent" :key="h.id" :handoff="h" />
          </ul>
          <p v-else class="border-t border-line px-5 py-5 text-[14px] text-ink3">No work handed to or from {{ m.name }} yet.</p>
        </Panel>

        <Panel title="Memory" :icon="Brain" :count="memories?.length || undefined" :to="`/memory?agent=${m.id}`">
          <div v-if="memories?.length" class="border-t border-line">
            <MemoryRows :memories="memories.slice(0, 6)" :names="names" />
          </div>
          <p v-else class="border-t border-line px-5 py-5 text-[14px] text-ink3">{{ m.name }} remembers nothing yet.</p>
        </Panel>

        <Panel v-if="mine.length" title="Routines" :icon="CalendarClock" :count="mine.length" to="/automation">
          <ul>
            <li v-for="r in mine" :key="r.id" class="flex items-center gap-3 border-t border-line px-5 py-3">
              <div class="min-w-0 flex-1">
                <p class="truncate text-[14px] font-medium">{{ r.title }}</p>
                <p class="truncate text-[12.5px] text-ink3">{{ triggers(r).map((t) => t.text).join(' · ') || 'Manual' }}<template v-if="r.next && r.status === 'active'"> · next {{ ago(r.next) }}</template></p>
              </div>
              <StatusTag :tone="routineLook(r).tone">{{ routineLook(r).label }}</StatusTag>
            </li>
          </ul>
        </Panel>
      </div>
    </div>
  </div>
</template>
