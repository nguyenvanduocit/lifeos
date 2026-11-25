<script setup lang="ts">
import { useRead } from '../data'
import AskChip from '../components/AskChip.vue'
import Empty from '../components/Empty.vue'
import PageHead from '../components/PageHead.vue'

const { data } = useRead('list_inbox')
</script>

<template>
  <PageHead title="Inbox" sub="Những thứ ghi vội, chưa rõ là việc, ghi chú hay người.">
    <AskChip words="Ghi vào inbox: " label="Ghi nhanh" />
    <AskChip v-if="data?.length" words="Phân loại inbox giúp mình." label="Phân loại tất cả" now />
  </PageHead>
  <ul v-if="data?.length" class="divide-y divide-line rounded-xl border border-line bg-panel">
    <li v-for="i in data" :key="i.id" class="flex items-center gap-3 px-4 py-2.5">
      <span class="flex-1 text-sm">{{ i.text }}</span>
      <span class="text-xs text-ink3">{{ i.created }}</span>
      <AskChip :words="`Xử lý mục inbox #${i.id} “${i.text}”: `" label="Xử lý" />
    </li>
  </ul>
  <Empty v-else-if="data" text="Inbox trống. Gõ bất cứ điều gì vào Assistant, agent sẽ xếp vào đúng chỗ." />
</template>
