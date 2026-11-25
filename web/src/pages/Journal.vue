<script setup lang="ts">
import { computed, ref } from 'vue'
import { BookOpen, CalendarDays } from '@lucide/vue'
import { useRead } from '../data'
import { longDay, todayIso } from '../format'
import AskChip from '../components/AskChip.vue'
import Empty from '../components/Empty.vue'
import Markdown from '../components/Markdown.vue'
import PageHead from '../components/PageHead.vue'
import Section from '../components/Section.vue'
import { Button } from '../components/ui/button'

const today = todayIso()
const picked = ref(today)
const { data: days, error: archiveError } = useRead('journal_recent', () => ({ limit: 60 }))
const { data: page, loading, error } = useRead('read_journal', () => ({ date: picked.value }))
const archive = computed(() => {
  const entries = days.value ?? []
  if (!entries.length || entries.some((entry) => entry.day === today)) return entries
  return [{ day: today, words: null }, ...entries]
})
</script>

<template>
  <div class="@container">
    <PageHead title="Nhật ký">
      <AskChip words="Viết nhật ký hôm nay: " label="Viết hôm nay" size="sm" primary />
      <AskChip v-if="days?.length" words="Đọc nhật ký 2 tuần qua và cho mình biết bạn thấy gì." label="Nhìn lại" size="sm" now />
    </PageHead>
    <p v-if="archiveError" role="alert" class="mb-4 text-sm text-critical">Không tải được các ngày đã ghi.</p>
    <div class="grid items-start gap-4" :class="{ '@min-[42rem]:grid-cols-[12rem_minmax(0,1fr)]': archive.length }">
      <Section v-if="archive.length" title="Lịch sử" :icon="CalendarDays">
        <ul class="flex max-h-80 gap-1 overflow-auto @min-[42rem]:max-h-[28rem] @min-[42rem]:flex-col">
          <li v-for="entry in archive" :key="entry.day" class="shrink-0">
            <Button
              type="button"
              size="sm"
              :variant="picked === entry.day ? 'secondary' : 'ghost'"
              :aria-pressed="picked === entry.day"
              class="h-auto w-full justify-start px-3 py-2 text-left"
              @click="picked = entry.day"
            >
              <span>
                <span class="block">{{ entry.day === today ? 'Hôm nay' : longDay(entry.day) }}</span>
                <span v-if="entry.words !== null" class="block text-xs font-normal text-ink2">{{ entry.words }} chữ</span>
              </span>
            </Button>
          </li>
        </ul>
      </Section>
      <Section :title="longDay(picked)" :icon="BookOpen">
        <p v-if="loading" role="status" class="py-2 text-sm text-ink2">Đang tải nhật ký…</p>
        <p v-else-if="error" role="alert" class="py-2 text-sm text-critical">Không tải được nhật ký ngày này.</p>
        <Markdown v-else-if="page?.body" :source="page.body" class="max-w-[72ch]" />
        <Empty v-else :text="picked === today ? 'Hôm nay chưa có ghi chép.' : 'Ngày này chưa có ghi chép.'" />
      </Section>
    </div>
  </div>
</template>
