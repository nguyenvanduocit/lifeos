<script setup lang="ts">
// The conversation with the agent, beside every page. Everything that
// changes a life happens here; the pages only show the result.
import { nextTick, onMounted, ref, watch } from 'vue'
import { Loader2, RotateCcw, Send, Wrench, X } from '@lucide/vue'
import Markdown from './Markdown.vue'
import { assistant, loadHistory, newConversation, sendDraft, setOpen, toolWords } from '../assistant'

const box = ref<HTMLTextAreaElement>()
const scroller = ref<HTMLElement>()

onMounted(loadHistory)

watch(
  () => assistant.focusTick,
  () => {
    box.value?.focus()
    const n = assistant.draft.length
    box.value?.setSelectionRange(n, n)
  },
)

function stick() {
  void nextTick(() => {
    const el = scroller.value
    if (el) el.scrollTop = el.scrollHeight
  })
}
watch(() => [assistant.turns.length, assistant.pending?.steps.length, assistant.pending?.steps.at(-1)?.text.length], stick)
onMounted(stick)

function key(e: KeyboardEvent) {
  if (e.key === 'Enter' && !e.shiftKey && !e.isComposing) {
    e.preventDefault()
    if (!assistant.pending) sendDraft()
  }
}

const SUGGESTIONS = [
  'Hôm nay mình nên làm gì?',
  'Vừa chi 50k ăn sáng',
  'Tối qua ngủ 7 tiếng',
  'Nhìn lại tuần này giúp mình',
]
</script>

<template>
  <aside class="flex h-full w-full flex-col border-l border-line bg-panel">
    <header class="flex items-center gap-2 border-b border-line px-4 py-3">
      <h2 class="text-sm font-semibold">Assistant</h2>
      <span class="text-xs text-ink3">mọi thay đổi đi qua đây</span>
      <button type="button" class="ml-auto text-ink3 hover:text-ink" title="Cuộc trò chuyện mới" @click="newConversation">
        <RotateCcw :size="15" />
      </button>
      <button type="button" class="text-ink3 hover:text-ink" title="Đóng" @click="setOpen(false)">
        <X :size="16" />
      </button>
    </header>

    <div ref="scroller" class="flex-1 space-y-4 overflow-y-auto px-4 py-4 text-sm">
      <div v-if="!assistant.turns.length && !assistant.pending" class="space-y-3 text-ink2">
        <p>Kể cho lifeos điều vừa xảy ra, hoặc nhờ nó sắp xếp. Agent ghi lại, dashboard cập nhật ngay.</p>
        <div class="flex flex-wrap gap-1.5">
          <button
            v-for="s in SUGGESTIONS"
            :key="s"
            type="button"
            class="rounded-full border border-line px-2.5 py-1 text-xs hover:border-accent hover:text-accent"
            @click="assistant.draft = s; box?.focus()"
          >
            {{ s }}
          </button>
        </div>
      </div>

      <template v-for="(t, i) in assistant.turns" :key="i">
        <div class="ml-8 rounded-lg bg-accent-soft/50 px-3 py-2 whitespace-pre-wrap">{{ t.ask }}</div>
        <div class="space-y-1.5">
          <div v-if="t.reply.tools.length" class="flex flex-wrap gap-1">
            <span v-for="(tool, j) in t.reply.tools" :key="j" class="inline-flex items-center gap-1 rounded bg-panel2 px-1.5 py-0.5 text-xs text-ink2">
              <Wrench :size="11" /> {{ toolWords(tool) }}
            </span>
          </div>
          <Markdown :source="t.reply.text" />
        </div>
      </template>

      <template v-if="assistant.pending">
        <div class="ml-8 rounded-lg bg-accent-soft/50 px-3 py-2 whitespace-pre-wrap">{{ assistant.pending.ask }}</div>
        <div class="space-y-1.5">
          <template v-for="(s, j) in assistant.pending.steps" :key="j">
            <span v-if="s.kind === 'tool'" class="mr-1 inline-flex items-center gap-1 rounded bg-panel2 px-1.5 py-0.5 text-xs text-ink2">
              <Wrench :size="11" /> {{ toolWords(s.text) }}
            </span>
            <Markdown v-else :source="s.text" />
          </template>
          <div class="flex items-center gap-1.5 text-xs text-ink3"><Loader2 :size="13" class="animate-spin" /> đang làm…</div>
        </div>
      </template>

      <p v-if="assistant.error" class="rounded-lg border border-critical/40 px-3 py-2 text-critical">{{ assistant.error }}</p>
    </div>

    <form class="border-t border-line p-3" @submit.prevent="sendDraft">
      <div class="flex items-end gap-2 rounded-lg border border-line bg-bg px-3 py-2 focus-within:border-accent">
        <textarea
          ref="box"
          v-model="assistant.draft"
          rows="2"
          class="max-h-40 flex-1 resize-none bg-transparent text-sm outline-none placeholder:text-ink3"
          placeholder="Nói với lifeos… (Enter để gửi)"
          @keydown="key"
        />
        <button type="submit" class="text-accent disabled:text-ink3" :disabled="!!assistant.pending || !assistant.draft.trim()" title="Gửi">
          <Send :size="16" />
        </button>
      </div>
    </form>
  </aside>
</template>
