<script setup lang="ts">
// The box under a conversation. Enter sends, Shift+Enter breaks the line;
// while the agent works, Stop ends its turn.
import { nextTick, ref, watch } from 'vue'
import { ArrowUp, Square } from '@lucide/vue'
import { conversation, panel, sendDraft, stopAgent } from '../../assistant'

const props = defineProps<{ agent: string; name: string; disabled?: string }>()
const box = ref<HTMLTextAreaElement>()
const c = () => conversation(props.agent)

function grow() {
  const el = box.value
  if (!el) return
  el.style.height = 'auto'
  el.style.height = Math.min(el.scrollHeight, 192) + 'px'
}
watch(() => c().draft, () => nextTick(grow))
watch(
  () => panel.focusTick,
  () => {
    box.value?.focus()
    const n = c().draft.length
    box.value?.setSelectionRange(n, n)
  },
)

function key(e: KeyboardEvent) {
  if (e.key === 'Enter' && !e.shiftKey && !e.isComposing) {
    e.preventDefault()
    if (!c().pending && c().draft.trim()) sendDraft(props.agent)
  } else if (e.key === 'Escape') {
    box.value?.blur()
  }
}
</script>

<template>
  <div class="p-3">
    <p v-if="disabled" class="rounded-xl border border-line bg-panel2 px-3 py-2.5 text-xs text-ink2">{{ disabled }}</p>
    <div
      v-else
      class="rounded-xl border border-line bg-panel shadow-sm focus-within:border-violet focus-within:ring-3 focus-within:ring-violet/15"
    >
      <textarea
        ref="box"
        v-model="c().draft"
        rows="1"
        class="block max-h-48 w-full resize-none bg-transparent px-3 pt-2.5 text-[13px] outline-none placeholder:text-ink3"
        :placeholder="`Nhắn ${name}…`"
        @keydown="key"
      />
      <div class="flex items-center gap-2 px-2 pb-2">
        <span class="text-[11px] text-ink3">{{ c().pending ? 'Đang làm; tin mới gửi sau khi xong' : 'Enter để gửi · Shift+Enter xuống dòng' }}</span>
        <button
          v-if="c().pending"
          type="button"
          class="ml-auto inline-flex items-center gap-1 rounded-full border border-line px-2.5 py-1 text-xs text-ink2 hover:border-critical hover:text-critical"
          title="Dừng agent"
          @click="stopAgent(agent)"
        >
          <Square :size="11" /> Dừng
        </button>
        <button
          type="button"
          class="inline-flex size-7 items-center justify-center rounded-full bg-violet text-white disabled:bg-panel3 disabled:text-ink3"
          :class="c().pending ? '' : 'ml-auto'"
          :disabled="!!c().pending || !c().draft.trim()"
          title="Gửi"
          @click="sendDraft(agent)"
        >
          <ArrowUp :size="15" />
        </button>
      </div>
    </div>
  </div>
</template>
