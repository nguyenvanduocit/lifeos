<script setup lang="ts">
import { ref } from 'vue'
import { Search } from '@lucide/vue'
import { useRead } from '../data'
import AskChip from '../components/AskChip.vue'
import Empty from '../components/Empty.vue'
import PageHead from '../components/PageHead.vue'
import Section from '../components/Section.vue'

const query = ref('')
const { data: notes } = useRead('recent_notes', () => ({ limit: 100 }))
const { data: reviews } = useRead('reviews', () => ({ limit: 20 }))
const { data: hits } = useRead('search', () => ({ query: query.value, limit: 20 }))
const KIND: Record<string, string> = { task: 'việc', goal: 'mục tiêu', person: 'người', journal: 'nhật ký', note: 'ghi chú', review: 'review' }
</script>

<template>
  <PageHead title="Ghi chú" sub="Ý tưởng, bài học, sách, và các bản review agent viết.">
    <AskChip words="Lưu ghi chú: " label="Ghi chú mới" />
  </PageHead>
  <label class="mb-4 flex items-center gap-2 rounded-lg border border-line bg-panel px-3 py-2 focus-within:border-accent">
    <Search :size="16" class="text-ink3" />
    <input v-model="query" class="flex-1 bg-transparent text-sm outline-none" placeholder="Tìm trong toàn bộ vault (không cần dấu)…" />
  </label>
  <Section v-if="query.trim()" title="Kết quả" :count="hits?.length" class="mb-4">
    <ul v-if="hits?.length" class="space-y-2 text-sm">
      <li v-for="h in hits" :key="h.path">
        <RouterLink :to="h.kind === 'person' ? `/people/${h.id}` : `/doc/${h.id}`" class="font-medium hover:underline">{{ h.title }}</RouterLink>
        <span class="ml-2 text-xs text-ink3">{{ KIND[h.kind] }}</span>
        <p class="text-xs text-ink2">{{ h.snippet.replace(/[«»]/g, '') }}</p>
      </li>
    </ul>
    <Empty v-else text="Không thấy." />
  </Section>
  <div class="grid gap-4 md:grid-cols-2">
    <Section title="Ghi chú" :count="notes?.length">
      <ul v-if="notes?.length" class="space-y-1 text-sm">
        <li v-for="n in notes" :key="n.id" class="flex gap-2">
          <RouterLink :to="`/doc/${n.id}`" class="flex-1 truncate hover:underline">{{ n.title }}</RouterLink>
          <span v-for="t in n.tags" :key="t" class="text-xs text-ink3">#{{ t }}</span>
        </li>
      </ul>
      <Empty v-else text="Chưa có ghi chú." />
    </Section>
    <Section title="Bản tin và review" :count="reviews?.length">
      <ul v-if="reviews?.length" class="space-y-1 text-sm">
        <li v-for="r in reviews" :key="r.id"><RouterLink :to="`/doc/${r.id}`" class="hover:underline">{{ r.title }}</RouterLink> <span class="text-xs text-ink3">{{ r.id.slice(0, 10) }}</span></li>
      </ul>
      <Empty v-else text="Chưa có." />
    </Section>
  </div>
</template>
