<script setup lang="ts">
// Consecutive tool calls as one row ("Dùng 3 tool · thêm việc, ghi thói quen ×2");
// open it for each call. Opens itself while it is the agent's live work.
import { computed, ref, watch } from 'vue'
import { ChevronRight, Wrench } from '@lucide/vue'
import CallCard from './CallCard.vue'
import { failed, summary, type Call } from './blocks'

const props = defineProps<{ calls: Call[]; live?: boolean }>()
const running = computed(() => props.calls.some((c) => !c.outcome))
const failures = computed(() => failed(props.calls))
const open = ref(!!props.live || failures.value > 0)
watch(() => props.live, (live) => { if (!live && !failures.value) open.value = false })
</script>

<template>
  <div>
    <button type="button" class="flex w-full items-center gap-2 py-0.5 text-left text-xs text-ink2 hover:text-ink" @click="open = !open">
      <ChevronRight :size="12" class="shrink-0 text-ink3 transition" :class="open ? 'rotate-90' : ''" />
      <Wrench :size="12" class="shrink-0 text-ink3" />
      <span v-if="running" class="sheen font-medium">Đang dùng tool…</span>
      <span v-else class="font-medium">Dùng {{ calls.length }} tool</span>
      <span v-if="failures" class="text-critical">{{ failures }} lỗi</span>
      <span class="truncate text-ink3">{{ summary(calls) }}</span>
    </button>
    <div v-if="open" class="mt-1.5 space-y-1 pl-5">
      <CallCard v-for="c in calls" :key="c.call.id" :item="c" />
    </div>
  </div>
</template>
