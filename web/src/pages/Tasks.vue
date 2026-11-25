<script setup lang="ts">
import { ref } from 'vue'
import { useRead } from '../data'
import AskChip from '../components/AskChip.vue'
import Empty from '../components/Empty.vue'
import PageHead from '../components/PageHead.vue'
import Section from '../components/Section.vue'
import TaskRow from '../components/TaskRow.vue'

const { data: v } = useRead('tasks_view')
const showDone = ref(false)
const { data: done } = useRead('list_tasks', () => ({ status: showDone.value ? 'done' : 'open' }))
</script>

<template>
  <PageHead title="Việc" sub="Hạn thật là “hạn”; ngày định làm là lịch. Muốn đổi gì, nói với agent.">
    <AskChip words="Thêm việc: " label="Thêm việc" />
    <AskChip words="Sắp xếp lại các việc tuần này giúp mình, dời những việc không kịp." label="Sắp xếp tuần" />
  </PageHead>
  <div v-if="v" class="grid gap-4 md:grid-cols-2">
    <Section v-if="v.overdue.length" title="Quá hạn" :count="v.overdue.length">
      <template #actions><AskChip words="Các việc quá hạn: việc nào nên làm ngay, việc nào dời, việc nào bỏ?" label="Xử lý" /></template>
      <ul><TaskRow v-for="t in v.overdue" :key="t.id" :task="t" /></ul>
    </Section>
    <Section title="Hôm nay" :count="v.today.length">
      <ul v-if="v.today.length || v.done_today.length">
        <TaskRow v-for="t in v.today" :key="t.id" :task="t" />
        <TaskRow v-for="t in v.done_today" :key="t.id" :task="t" done />
      </ul>
      <Empty v-else text="Không có việc nào cho hôm nay." />
    </Section>
    <Section title="7 ngày tới" :count="v.upcoming.length">
      <ul v-if="v.upcoming.length"><TaskRow v-for="t in v.upcoming" :key="t.id" :task="t" /></ul>
      <Empty v-else text="Tuần tới còn trống." />
    </Section>
    <Section title="Chưa xếp lịch" :count="v.anytime.length">
      <ul v-if="v.anytime.length"><TaskRow v-for="t in v.anytime" :key="t.id" :task="t" /></ul>
      <Empty v-else text="Không có." />
    </Section>
    <Section title="Đã xong" class="md:col-span-2">
      <template #actions>
        <button type="button" class="text-xs text-ink3 hover:text-ink" @click="showDone = !showDone">{{ showDone ? 'Ẩn' : 'Hiện tất cả' }}</button>
      </template>
      <ul v-if="showDone && done"><TaskRow v-for="t in done" :key="t.id" :task="t" done /></ul>
    </Section>
  </div>
</template>
