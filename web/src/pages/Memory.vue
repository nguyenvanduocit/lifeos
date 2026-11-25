<script setup lang="ts">
// Everything lifeos's agents remember, read-only. To forget or correct,
// tell the agent; it changes the memory and this page follows.
import { computed, ref } from 'vue'
import { useRead } from '../data'
import { ASSISTANT } from '../assistant'
import AskChip from '../components/AskChip.vue'
import PageHead from '../components/PageHead.vue'
import MemoryList from '../components/chat/MemoryList.vue'

const { data: crew } = useRead('list_crew')
const holder = ref<string | undefined>(undefined)
const names = computed(() => Object.fromEntries([[ASSISTANT, 'Assistant'], ...(crew.value ?? []).map((m) => [m.id, m.name])]))
</script>

<template>
  <PageHead title="Memory" sub="Những gì agent nhớ về bạn qua mọi cuộc trò chuyện. Trước mỗi tin nhắn, lifeos tự gợi lại những điều liên quan.">
    <AskChip words="Hãy nhớ rằng " label="Dạy agent một điều" />
    <AskChip words="Rà lại memory: gộp những điều trùng, quên những điều đã cũ." label="Dọn memory" now />
  </PageHead>
  <div class="mb-4 flex flex-wrap gap-1 text-xs" role="group" aria-label="Của ai">
    <button
      v-for="h in [undefined, ASSISTANT, ...(crew ?? []).map((m) => m.id)]"
      :key="h ?? 'all'"
      type="button"
      class="rounded-full border px-2.5 py-0.5"
      :class="holder === h ? 'border-violet text-violet-text' : 'border-line text-ink2'"
      @click="holder = h"
    >
      {{ h ? names[h] : 'Tất cả' }}
    </button>
  </div>
  <div class="max-w-3xl">
    <MemoryList :key="holder ?? 'all'" :holder="holder" :names="names" show-holder />
  </div>
</template>
