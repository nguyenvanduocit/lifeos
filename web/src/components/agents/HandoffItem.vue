<script setup lang="ts">
import { computed, type Component } from 'vue'
import {
  ArrowRight, CircleCheck, CircleDashed, CornerDownRight, Hourglass, LoaderCircle, Megaphone, MessageCircle, MessageCircleQuestion, Undo2,
  Wrench, XCircle,
} from '@lucide/vue'
import { talkTo } from '../../assistant'
import AskChip from '../AskChip.vue'
import StatusTag from '../ds/StatusTag.vue'
import AgentBadge from '../chat/AgentBadge.vue'
import Markdown from '../assistant/Markdown.vue'
import { ago, plain, when } from './when'
import type { Handoff } from '../../core/generated/Handoff'

// One thing an agent asked or handed another (docs/adr/0002) and, nested under it, what the agent that had it handed on
// for it: who to whom, what kind, how it stands, the words that went over and what came back. Folded it shows a line of
// each; opened, all of it, with the chain it passed through and what lifeos's health check saw. Nothing here acts on the
// agents: the owner opens a chat, or drafts a word to the Assistant to take one back.
const props = defineProps<{ handoff: Handoff; kids: ReadonlyMap<number, readonly Handoff[]>; open: ReadonlySet<number>; depth?: number }>()
const emit = defineEmits<{ toggle: [id: number] }>()

const h = computed(() => props.handoff)
const children = computed(() => props.kids.get(h.value.id) ?? [])
const opened = computed(() => props.open.has(h.value.id))

interface Look { label: string; tone: string; icon: Component; spin?: boolean }
const STATUS: Record<Handoff['status'], Look> = {
  queued: { label: 'Queued', tone: 'neutral', icon: CircleDashed },
  working: { label: 'Working', tone: 'talk', icon: LoaderCircle, spin: true },
  waiting: { label: 'Waiting on what it handed on', tone: 'warning', icon: Hourglass },
  done: { label: 'Done', tone: 'good', icon: CircleCheck },
  failed: { label: 'Failed', tone: 'critical', icon: XCircle },
  cancelled: { label: 'Taken back', tone: 'neutral', icon: Undo2 },
}
const KIND: Record<Handoff['kind'], { label: string; icon: Component }> = {
  ask: { label: 'Question', icon: MessageCircleQuestion },
  task: { label: 'Task', icon: Wrench },
  news: { label: 'News', icon: Megaphone },
}
const look = computed((): Look => (h.value.kind === 'news' ? { label: 'Told', tone: 'neutral', icon: Megaphone } : STATUS[h.value.status]))
const isOpen = computed(() => ['queued', 'working', 'waiting'].includes(h.value.status))

// How long it was out: from sent to closed, or to now while out.
const took = computed(() => {
  const start = Date.parse(h.value.created)
  const end = h.value.closed ? Date.parse(h.value.closed) : Date.now()
  if (isNaN(start) || isNaN(end) || h.value.kind === 'news') return ''
  const mins = Math.max(0, Math.round((end - start) / 60_000))
  if (mins < 1) return '<1m'
  return mins < 60 ? `${mins}m` : `${Math.floor(mins / 60)}h ${mins % 60}m`
})
</script>

<template>
  <li :data-handoff="h.id" :data-status="h.status" class="min-w-0">
    <div class="rounded-xl border bg-panel transition-colors" :class="opened ? 'border-tone-line' : 'border-line hover:border-tone-line'" data-tone="talk">
      <button type="button" class="flex w-full min-w-0 items-start gap-3 p-3 text-left" :aria-expanded="opened" @click="emit('toggle', h.id)">
        <span class="mt-0.5 flex shrink-0 items-center gap-1">
          <AgentBadge :id="h.sender" :name="h.sender_name" :size="22" />
          <ArrowRight class="size-3.5 text-ink3" aria-hidden="true" />
          <AgentBadge :id="h.target" :name="h.target_name" :size="22" />
        </span>
        <span class="min-w-0 flex-1">
          <span class="flex flex-wrap items-center gap-x-1.5 gap-y-1 text-sm">
            <b class="font-semibold">{{ h.sender_name }}</b><span class="text-ink3" aria-hidden="true">→</span><b class="font-semibold">{{ h.target_name }}</b>
            <span class="inline-flex items-center gap-1 text-xs text-ink2"><component :is="KIND[h.kind].icon" class="size-3" aria-hidden="true" />{{ KIND[h.kind].label }}</span>
            <StatusTag :tone="look.tone" :pulse="look.spin">{{ look.label }}</StatusTag>
            <span v-if="h.note && isOpen" data-tone="warning" class="text-xs text-tone-ink">{{ h.note }}</span>
          </span>
          <span class="mt-1 block text-sm wrap-anywhere text-ink2" :class="opened ? '' : 'line-clamp-1'">{{ plain(h.text) }}</span>
          <span v-if="!opened && h.result" class="mt-0.5 flex min-w-0 items-start gap-1 text-xs text-ink3">
            <CornerDownRight class="mt-0.5 size-3 shrink-0" aria-hidden="true" /><span class="line-clamp-1 wrap-anywhere">{{ plain(h.result) }}</span>
          </span>
        </span>
        <span class="flex shrink-0 flex-col items-end gap-0.5 text-xs text-ink2 tabular-nums">
          <time :datetime="h.created" :title="when(h.created)">{{ ago(h.created) }}</time>
          <span v-if="took" class="text-ink3" :title="isOpen ? 'Out for' : 'Took'">{{ took }}</span>
          <span class="text-ink3">#{{ h.id }}</span>
        </span>
      </button>

      <div v-if="opened" class="space-y-3 border-t border-line px-3 pt-3 pb-3 text-sm">
        <section>
          <h4 class="mb-1 text-xs font-semibold text-ink2">{{ h.sender_name }} {{ h.kind === 'ask' ? 'asked' : h.kind === 'news' ? 'told' : 'handed over' }}</h4>
          <div class="rounded-lg bg-panel2 px-3 py-2"><Markdown type="body" :text="h.text" wrap /></div>
        </section>
        <section v-if="h.kind !== 'news'">
          <h4 class="mb-1 text-xs font-semibold text-ink2">
            <template v-if="h.status === 'waiting'">{{ h.target_name }}'s answer so far, held until what it handed on comes back</template>
            <template v-else-if="isOpen">Nothing has come back yet</template>
            <template v-else-if="h.status === 'failed'">It could not be done<template v-if="h.note">: {{ h.note }}</template></template>
            <template v-else-if="h.status === 'cancelled'">Taken back<template v-if="h.note"> ({{ h.note }})</template></template>
            <template v-else>{{ h.target_name }} answered<template v-if="!h.reported">, on its way to {{ h.sender_name }}</template></template>
          </h4>
          <div v-if="h.result" class="rounded-lg bg-panel2 px-3 py-2"><Markdown type="body" :text="h.result" wrap /></div>
        </section>
        <dl class="grid grid-cols-[auto_1fr] gap-x-3 gap-y-1 text-xs text-ink2">
          <template v-if="h.chain.length > 1"><dt class="text-ink3">Chain</dt><dd>{{ [...h.chain, h.target].join(' → ') }}</dd></template>
          <template v-if="h.attempts > 1"><dt class="text-ink3">Attempts</dt><dd>{{ h.attempts }} (lifeos handed it again)</dd></template>
          <template v-if="h.note && isOpen"><dt class="text-ink3">Health</dt><dd>{{ h.note }}</dd></template>
          <dt class="text-ink3">Sent</dt><dd>{{ when(h.created) }}</dd>
          <template v-if="h.closed && h.kind !== 'news'"><dt class="text-ink3">Closed</dt><dd>{{ when(h.closed) }}</dd></template>
        </dl>
        <div class="flex flex-wrap items-center gap-1.5 border-t border-line pt-2">
          <button type="button" class="inline-flex h-7 items-center gap-1 rounded-full px-2.5 text-xs font-semibold text-tone-ink hover:bg-tone-wash" @click="talkTo(h.sender)">
            <MessageCircle class="size-3.5" aria-hidden="true" /> {{ h.sender_name }}'s chat
          </button>
          <button type="button" class="inline-flex h-7 items-center gap-1 rounded-full px-2.5 text-xs font-semibold text-tone-ink hover:bg-tone-wash" @click="talkTo(h.target)">
            <MessageCircle class="size-3.5" aria-hidden="true" /> {{ h.target_name }}'s chat
          </button>
          <AskChip v-if="isOpen" :icon="Undo2" label="Take back" :words="`Take back hand-off #${h.id} (cancel_handoff), because `" />
        </div>
      </div>
    </div>

    <ul v-if="children.length" class="mt-2 ml-4 space-y-2 border-l-2 border-line pl-3" :aria-label="`Handed on by ${h.target_name}`">
      <HandoffItem v-for="c in children" :key="c.id" :handoff="c" :kids="kids" :open="open" :depth="(depth ?? 0) + 1" @toggle="emit('toggle', $event)" />
    </ul>
  </li>
</template>
