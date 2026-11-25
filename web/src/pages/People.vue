<script setup lang="ts">
import { computed } from 'vue'
import { useRead } from '../data'
import AskChip from '../components/AskChip.vue'
import Empty from '../components/Empty.vue'
import PageHead from '../components/PageHead.vue'
import PersonRow from '../components/PersonRow.vue'
import Section from '../components/Section.vue'

const { data } = useRead('list_people')
const due = computed(() => data.value?.filter((p) => p.overdue) ?? [])
const birthdays = computed(() =>
  (data.value ?? []).filter((p) => p.days_to_birthday !== null && p.days_to_birthday <= 60).sort((a, b) => (a.days_to_birthday ?? 0) - (b.days_to_birthday ?? 0)),
)
const groups = computed(() => {
  const m = new Map<string, typeof due.value>()
  for (const p of data.value ?? []) {
    const k = p.relation ?? 'khác'
    m.set(k, [...(m.get(k) ?? []), p])
  }
  return [...m.entries()].sort((a, b) => a[0].localeCompare(b[0], 'vi'))
})
</script>

<template>
  <PageHead title="Mối quan hệ" sub="Ai cần liên lạc, sinh nhật sắp tới, và những gì lifeos nhớ về từng người.">
    <AskChip words="Thêm người: " label="Thêm người" />
    <AskChip words="Vừa gặp " label="Ghi lần gặp" />
  </PageHead>
  <div v-if="data" class="grid gap-4 md:grid-cols-2">
    <Section title="Nên liên lạc" :count="due.length">
      <ul v-if="due.length"><PersonRow v-for="p in due" :key="p.id" :person="p" /></ul>
      <Empty v-else text="Bạn đang giữ liên lạc tốt." />
    </Section>
    <Section title="Sinh nhật trong 60 ngày">
      <ul v-if="birthdays.length"><PersonRow v-for="p in birthdays" :key="p.id" :person="p" show="birthday" /></ul>
      <Empty v-else text="Không có." />
    </Section>
    <Section v-for="[rel, people] in groups" :key="rel" :title="rel" :count="people.length" class="first-letter:uppercase">
      <ul class="normal-case"><PersonRow v-for="p in people" :key="p.id" :person="p" /></ul>
    </Section>
    <Empty v-if="!data.length" text="lifeos chưa biết ai trong đời bạn." words="Thêm người: " label="Thêm người đầu tiên" />
  </div>
</template>
