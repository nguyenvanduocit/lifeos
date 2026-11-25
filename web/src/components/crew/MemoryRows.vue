<script setup lang="ts">
// What agents remember, as rows (the Memory page and an agent's page): its kind as a mark, the title and a line of it,
// who holds it, how often it was recalled and whether it helped. A row opens to the whole memory and what it rests on.
// Forgetting is asked of the agent that holds it (`say`), never done here.
import { computed, reactive } from 'vue'
import { kindOf } from './kinds'
import { BookmarkCheck, ChevronDown, ThumbsDown, ThumbsUp } from '@lucide/vue'
import type { Memory } from '../../core/generated/Memory'
import { say } from '../../assistant'
import AgentBadge from '../chat/AgentBadge.vue'
import IconTile from '../ds/IconTile.vue'
import StatusTag from '../ds/StatusTag.vue'
import { ago } from '../agents/when'

const props = defineProps<{ memories: readonly Memory[]; names?: Record<string, string>; showHolder?: boolean }>()

const open = reactive(new Set<number>())
const toggle = (id: number) => (open.has(id) ? open.delete(id) : open.add(id))
const rows = computed(() => props.memories)
function forget(x: Memory) {
  say(`Forget memory #${x.id} “${x.title}” because `, x.holder)
}
</script>

<template>
  <ul>
    <li v-for="x in rows" :key="x.id" class="border-b border-line last:border-b-0" :class="x.ended ? 'opacity-60' : ''">
      <button type="button" class="flex w-full min-w-0 items-start gap-3.5 px-5 py-3.5 text-left transition-colors hover:bg-panel2/60" :aria-expanded="open.has(x.id)" @click="toggle(x.id)">
        <IconTile :icon="kindOf(x.kind).icon" :tone="kindOf(x.kind).tone" size="sm" class="mt-0.5" />
        <span class="min-w-0 flex-1">
          <span class="flex flex-wrap items-center gap-x-2 gap-y-1">
            <span class="text-[14.5px] font-medium wrap-anywhere" :class="x.ended ? 'line-through' : ''">{{ x.title }}</span>
            <StatusTag v-if="x.trust === 'confirmed'" tone="good">Confirmed</StatusTag>
            <StatusTag v-if="x.ended" tone="neutral">Forgotten</StatusTag>
          </span>
          <span v-if="x.body" class="mt-0.5 block text-[13.5px] wrap-anywhere text-ink2" :class="open.has(x.id) ? 'whitespace-pre-wrap' : 'line-clamp-1'">{{ x.body }}</span>
          <span class="mt-1.5 flex flex-wrap items-center gap-x-3 gap-y-1 text-[12.5px] text-ink3">
            <span>{{ kindOf(x.kind).label }}</span>
            <span v-if="showHolder" class="inline-flex items-center gap-1.5 text-ink2"><AgentBadge :id="x.holder" :name="names?.[x.holder] ?? x.holder" :size="14" />{{ names?.[x.holder] ?? x.holder }}</span>
            <span v-if="x.recalled" class="tabular-nums">Recalled {{ x.recalled }}×</span>
            <span v-if="x.helped" class="inline-flex items-center gap-1 tabular-nums"><ThumbsUp class="size-3" aria-hidden="true" />{{ x.helped }}<span class="sr-only">helped</span></span>
            <span v-if="x.misled" class="inline-flex items-center gap-1 text-critical tabular-nums"><ThumbsDown class="size-3" aria-hidden="true" />{{ x.misled }}<span class="sr-only">misled</span></span>
            <span :title="x.updated">Updated {{ ago(x.updated) }}</span>
          </span>
        </span>
        <ChevronDown class="mt-1 size-4 shrink-0 text-ink3 transition-transform" :class="open.has(x.id) ? 'rotate-180' : ''" aria-hidden="true" />
      </button>
      <div v-if="open.has(x.id)" class="space-y-3 px-5 pb-4 pl-[3.85rem] text-[13px]">
        <blockquote v-if="x.evidence" class="rounded-xl border border-line bg-panel2 px-3.5 py-2.5 text-ink2">
          <span class="mb-1 flex items-center gap-1.5 text-[12px] font-medium text-ink3"><BookmarkCheck class="size-3.5" aria-hidden="true" /> Learned from what you said</span>
          “{{ x.evidence }}”
        </blockquote>
        <dl class="grid grid-cols-[7rem_1fr] gap-y-1 text-ink2">
          <dt class="text-ink3">Memory</dt><dd class="tabular-nums">#{{ x.id }}</dd>
          <dt class="text-ink3">Written</dt><dd>{{ x.created }}</dd>
          <template v-if="x.last_recalled"><dt class="text-ink3">Last recalled</dt><dd>{{ x.last_recalled }}</dd></template>
          <template v-if="x.links.length"><dt class="text-ink3">Linked</dt><dd>{{ x.links.length }} other memor{{ x.links.length === 1 ? 'y' : 'ies' }}</dd></template>
          <template v-if="x.ended"><dt class="text-ink3">Forgotten</dt><dd>{{ x.ended }}<template v-if="x.why_ended"> · {{ x.why_ended }}</template></dd></template>
        </dl>
        <button v-if="!x.ended" type="button"
          class="inline-flex h-8 items-center gap-1.5 rounded-[10px] border border-line2 bg-panel px-3 text-[13px] font-medium hover:bg-panel2"
          title="Drafts this in the Assistant for you to send" @click="forget(x)">
          Forget…
        </button>
      </div>
    </li>
  </ul>
</template>
