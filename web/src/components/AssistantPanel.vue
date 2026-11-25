<script setup lang="ts">
// The conversations, beside every page: the Assistant and each crew member.
// Everything that changes a life happens here; pages only show the result.
import { computed, nextTick, onMounted, ref, watch } from 'vue'
import { ArrowDown, ChevronDown, RotateCcw, X } from '@lucide/vue'
import Markdown from './Markdown.vue'
import AgentBadge from './chat/AgentBadge.vue'
import Composer from './chat/Composer.vue'
import MemoryList from './chat/MemoryList.vue'
import TurnView from './chat/TurnView.vue'
import { ASSISTANT, conversation, newConversation, panel, setOpen, talkTo } from '../assistant'
import { useRead } from '../data'

const { data: crew } = useRead('list_crew')
const member = computed(() => crew.value?.find((m) => m.id === panel.agent))
const me = computed(() =>
  panel.agent === ASSISTANT ? { id: ASSISTANT, name: 'Assistant' } : { id: panel.agent, name: member.value?.name ?? panel.agent },
)
const c = computed(() => conversation(panel.agent))
const doing = computed(() => {
  const p = c.value.pending
  if (!p) return null
  const last = p.steps[p.steps.length - 1]
  if (last && 'Tool' in last) return `đang ${last.Tool.name}`
  if (last && 'Thinking' in last) return 'đang suy nghĩ'
  return 'đang làm'
})
const disabled = computed(() => (member.value?.status === 'retired' ? `${member.value.name} đã nghỉ. Nhờ Assistant gọi lại nếu cần.` : undefined))
const picking = ref(false)
const active = computed(() => (crew.value ?? []).filter((m) => m.status === 'active'))

// Follow new output while near the bottom; let go once the user scrolls up.
const feed = ref<HTMLElement>()
const pinned = ref(true)
function onScroll() {
  const el = feed.value
  if (el) pinned.value = el.scrollHeight - el.scrollTop - el.clientHeight < 60
}
function toBottom() {
  void nextTick(() => {
    const el = feed.value
    if (el) el.scrollTop = el.scrollHeight
    pinned.value = true
  })
}
const growth = computed(() => {
  const p = c.value.pending
  const tail = p?.steps[p.steps.length - 1]
  return `${panel.agent}:${c.value.turns.length}:${p?.steps.length ?? -1}:${tail && 'Text' in tail ? tail.Text.length : 0}`
})
watch(growth, () => { if (pinned.value) toBottom() })
watch(() => panel.agent, toBottom)
onMounted(toBottom)

const SUGGESTIONS = ['Hôm nay mình nên làm gì?', 'Vừa chi 50k ăn sáng', 'Tối qua ngủ 7 tiếng', 'Mình muốn có một huấn luyện viên sức khoẻ riêng']
</script>

<template>
  <aside class="flex h-full w-full flex-col border-l border-line bg-bg">
    <!-- Header: who, what it is doing, tabs -->
    <header class="border-b border-line bg-panel px-4 pt-3">
      <div class="flex items-center gap-2">
        <div class="relative min-w-0">
          <button type="button" class="flex items-center gap-2 rounded-md py-0.5 pr-1 hover:bg-panel2" @click="picking = !picking">
            <AgentBadge :id="me.id" :name="me.name" />
            <span class="truncate text-[14px] font-bold">{{ me.name }}</span>
            <ChevronDown :size="14" class="text-ink3" />
          </button>
          <div v-if="picking" class="absolute left-0 top-full z-30 mt-1 w-64 rounded-lg border border-line bg-panel p-1 shadow-lg" @mouseleave="picking = false">
            <button
              v-for="a in [{ id: ASSISTANT, name: 'Assistant', role: 'quản lý chung, thay đổi mọi thứ' }, ...active.map((m) => ({ id: m.id, name: m.name, role: m.role ?? '' }))]"
              :key="a.id"
              type="button"
              class="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left hover:bg-panel2"
              :class="a.id === panel.agent ? 'bg-panel2' : ''"
              @click="talkTo(a.id); picking = false"
            >
              <AgentBadge :id="a.id" :name="a.name" :size="20" />
              <span class="min-w-0">
                <span class="block truncate text-sm font-medium">{{ a.name }}</span>
                <span class="block truncate text-[11px] text-ink3">{{ a.role }}</span>
              </span>
              <span v-if="conversation(a.id).pending" class="ml-auto size-1.5 rounded-full bg-violet" aria-label="đang làm" />
            </button>
            <RouterLink to="/crew" class="block rounded-md px-2 py-1.5 text-xs text-ink3 hover:bg-panel2" @click="picking = false">Quản lý crew…</RouterLink>
          </div>
        </div>
        <button type="button" class="ml-auto text-ink3 hover:text-ink" title="Bắt đầu lại cuộc trò chuyện" @click="newConversation()">
          <RotateCcw :size="15" />
        </button>
        <button type="button" class="text-ink3 hover:text-ink" title="Đóng" @click="setOpen(false)"><X :size="16" /></button>
      </div>
      <div class="mt-1 flex items-center gap-1.5 text-xs" :class="doing ? 'text-violet-text' : 'text-ink3'">
        <span class="size-1.5 rounded-full" :class="doing ? 'animate-pulse bg-violet' : 'bg-good'" />
        {{ doing ?? (member?.role ?? 'sẵn sàng; mọi thay đổi đi qua đây') }}
      </div>
      <nav class="mt-2 flex gap-4 text-xs">
        <button
          v-for="t in [{ id: 'chat', label: 'Trò chuyện' }, { id: 'memory', label: 'Memory' }, ...(member ? [{ id: 'about', label: 'Vai trò' }] : [])]"
          :key="t.id"
          type="button"
          class="border-b-2 pb-2"
          :class="panel.tab === t.id ? 'border-violet font-semibold text-ink' : 'border-transparent text-ink3 hover:text-ink'"
          @click="panel.tab = t.id as typeof panel.tab"
        >
          {{ t.label }}
        </button>
      </nav>
    </header>

    <!-- Conversation -->
    <template v-if="panel.tab === 'chat'">
      <div class="relative min-h-0 flex-1">
        <div ref="feed" class="h-full space-y-6 overflow-y-auto px-5 py-4" @scroll="onScroll">
          <div v-if="c.loaded && !c.turns.length && !c.pending" class="space-y-3 text-sm text-ink2">
            <p v-if="member">{{ member.name }} sẵn sàng. Kể cho {{ member.name }} điều thuộc phần của mình.</p>
            <p v-else>Kể cho lifeos điều vừa xảy ra, hoặc nhờ nó sắp xếp. Agent ghi lại, dashboard cập nhật ngay.</p>
            <div v-if="!member" class="flex flex-wrap gap-1.5">
              <button
                v-for="s in SUGGESTIONS"
                :key="s"
                type="button"
                class="rounded-full border border-line px-2.5 py-1 text-xs hover:border-violet hover:text-violet-text"
                @click="c.draft = s; panel.focusTick++"
              >
                {{ s }}
              </button>
            </div>
          </div>
          <TurnView
            v-for="(t, i) in c.turns"
            :key="i"
            :ask="t.ask"
            :at="t.at"
            :steps="t.reply.steps"
            :done="t.reply"
            :agent="me"
          />
          <TurnView v-if="c.pending" :ask="c.pending.ask" :steps="c.pending.steps" :agent="me" live />
          <div v-if="c.error" class="rounded-lg border border-critical/40 bg-critical-wash px-3 py-2 text-xs text-critical">{{ c.error }}</div>
        </div>
        <button
          v-if="!pinned"
          type="button"
          class="absolute bottom-3 right-4 inline-flex items-center gap-1 rounded-full border border-line bg-panel px-2.5 py-1 text-xs shadow"
          @click="toBottom"
        >
          <ArrowDown :size="12" /> Mới nhất
        </button>
      </div>
      <Composer :agent="me.id" :name="me.name" :disabled="disabled" class="border-t border-line bg-panel" />
    </template>

    <div v-else-if="panel.tab === 'memory'" class="min-h-0 flex-1 overflow-y-auto px-4 py-4">
      <MemoryList :holder="me.id" />
    </div>

    <div v-else-if="member" class="min-h-0 flex-1 space-y-3 overflow-y-auto px-4 py-4 text-sm">
      <div class="flex flex-wrap gap-x-4 gap-y-1 text-xs text-ink2">
        <span>Vai trò: {{ member.role ?? '–' }}</span>
        <span>Model: {{ member.model ?? 'mặc định' }}</span>
        <span>Effort: {{ member.effort ?? 'mặc định' }}</span>
      </div>
      <div class="text-xs text-ink2">Tool: {{ member.tools.length ? member.tools.join(', ') : 'mọi tool của lifeos' }}</div>
      <Markdown :source="member.instructions || '_Chưa có hướng dẫn._'" class="rounded-lg border border-line bg-panel p-3" />
      <p class="text-xs text-ink3">Muốn đổi vai trò hay hướng dẫn, nói với Assistant.</p>
    </div>
  </aside>
</template>
