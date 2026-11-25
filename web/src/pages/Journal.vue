<script setup lang="ts">
import { ref } from 'vue'
import { useRead } from '../data'
import { longDay, todayIso } from '../format'
import AskChip from '../components/AskChip.vue'
import Empty from '../components/Empty.vue'
import Markdown from '../components/Markdown.vue'
import PageHead from '../components/PageHead.vue'

const { data: days } = useRead('journal_recent', () => ({ limit: 60 }))
const picked = ref(todayIso())
const { data: page } = useRead('read_journal', () => ({ date: picked.value }))
</script>

<template>
  <PageHead title="Nhật ký" sub="Kể cho agent, nó viết lại bằng giọng của bạn.">
    <AskChip words="Viết nhật ký hôm nay: " label="Viết" />
    <AskChip words="Đọc nhật ký 2 tuần qua và cho mình biết bạn thấy gì." label="Nhìn lại" now />
  </PageHead>
  <div class="grid gap-4 md:grid-cols-[220px_1fr]">
    <ul class="space-y-0.5 text-sm">
      <li v-for="d in days ?? []" :key="d.day">
        <button
          type="button"
          class="w-full rounded-lg px-2 py-1.5 text-left hover:bg-panel2"
          :class="picked === d.day ? 'bg-panel2 font-medium' : ''"
          @click="picked = d.day"
        >
          <div>{{ longDay(d.day) }}</div>
          <div class="text-xs text-ink3">{{ d.words }} chữ</div>
        </button>
      </li>
    </ul>
    <article class="min-h-60 rounded-xl border border-line bg-panel p-5">
      <h2 class="mb-3 text-sm font-semibold text-ink2">{{ longDay(picked) }}</h2>
      <Markdown v-if="page" :source="page.body" />
      <Empty v-else text="Ngày này chưa có gì." :words="picked === todayIso() ? 'Viết nhật ký hôm nay: ' : undefined" label="Viết" />
    </article>
  </div>
</template>
