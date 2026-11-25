<script setup lang="ts">
import { useRead } from '../data'
import AskChip from '../components/AskChip.vue'
import Empty from '../components/Empty.vue'
import PageHead from '../components/PageHead.vue'

const { data } = useRead('list_inbox')
</script>

<template>
  <PageHead title="Inbox" sub="Những thứ ghi vội, chưa rõ là việc, ghi chú hay người.">
    <AskChip size="sm" primary words="Ghi vào inbox: " label="Ghi nhanh" />
    <AskChip size="sm" v-if="data?.length" words="Phân loại inbox giúp mình." label="Phân loại tất cả" now />
  </PageHead>
  <ul v-if="data?.length" class="divide-y divide-line rounded-2xl border border-line bg-panel">
    <li v-for="i in data" :key="i.id" class="flex items-center gap-3 px-4 py-2.5">
      <span class="flex-1 text-sm">{{ i.text }}</span>
      <span class="text-xs text-ink3">{{ i.created }}</span>
      <AskChip :words="`Xử lý mục inbox #${i.id} “${i.text}”: `" label="Xử lý" />
    </li>
  </ul>
  <Empty v-else-if="data" class="rounded-2xl border border-line bg-panel p-5" text="Inbox trống." />
</template>
