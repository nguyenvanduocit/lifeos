<script setup lang="ts">
import { useRead } from '../data'
import AskChip from '../components/AskChip.vue'
import Markdown from '../components/Markdown.vue'
import PageHead from '../components/PageHead.vue'

const props = defineProps<{ id: string }>()
const { data, error } = useRead('read_doc', () => ({ id: props.id }))
const KIND: Record<string, string> = { task: 'Việc', goal: 'Mục tiêu', person: 'Người', journal: 'Nhật ký', note: 'Ghi chú', review: 'Review' }
</script>

<template>
  <p v-if="error" class="text-sm text-critical">{{ error }}</p>
  <template v-if="data">
    <PageHead :title="data.summary.title" :sub="[KIND[data.summary.kind], data.summary.status, data.summary.due && 'hạn ' + data.summary.due, data.summary.path].filter(Boolean).join(' · ')">
      <AskChip size="sm" :words="`Về “${data.summary.title}” (${data.summary.path}): `" label="Nhờ agent sửa" />
    </PageHead>
    <div v-if="data.summary.tags.length" class="mb-3 flex gap-2 text-xs text-ink3"><span v-for="t in data.summary.tags" :key="t">#{{ t }}</span></div>
    <article class="rounded-2xl border border-line bg-panel p-5">
      <Markdown v-if="data.body.trim()" :source="data.body" />
      <p v-else class="text-sm text-ink3">Trống.</p>
    </article>
  </template>
</template>
