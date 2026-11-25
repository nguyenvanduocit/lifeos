<script setup lang="ts">
import { Cake } from '@lucide/vue'
import { useRead } from '../data'
import { longDay } from '../format'
import AskChip from '../components/AskChip.vue'
import Markdown from '../components/Markdown.vue'
import PageHead from '../components/PageHead.vue'
import Section from '../components/Section.vue'

const props = defineProps<{ id: string }>()
const { data, error } = useRead('person', () => ({ id: props.id }))
</script>

<template>
  <p v-if="error" class="text-sm text-critical">{{ error }}</p>
  <template v-if="data">
    <PageHead :title="data.person.name" :sub="[data.person.relation, data.person.contact_every_days ? `liên lạc mỗi ${data.person.contact_every_days} ngày` : ''].filter(Boolean).join(' · ')">
      <AskChip size="sm" :words="`Vừa nói chuyện với ${data.person.name} (id ${data.person.id}): `" label="Ghi liên lạc" />
      <AskChip size="sm" :words="`Ghi nhớ về ${data.person.name} (id ${data.person.id}): `" label="Ghi nhớ" />
      <AskChip size="sm" :words="`Gợi ý mình nên hỏi thăm hay làm gì cho ${data.person.name} (id ${data.person.id}).`" label="Gợi ý" now />
    </PageHead>
    <div class="grid gap-4 md:grid-cols-3">
      <Section title="Những gì lifeos nhớ" class="md:col-span-2">
        <p v-if="data.person.birthday" class="mb-2 inline-flex items-center gap-1 text-sm text-ink2">
          <Cake :size="14" /> {{ data.person.birthday }}<template v-if="data.person.days_to_birthday !== null"> · {{ data.person.days_to_birthday }} ngày nữa</template>
        </p>
        <Markdown v-if="data.body.trim()" :source="data.body" class="text-sm" />
        <p v-else class="text-sm text-ink3">Chưa có ghi chú.</p>
      </Section>
      <Section title="Các lần liên lạc" :count="data.interactions.length">
        <ol v-if="data.interactions.length" class="space-y-2 text-sm">
          <li v-for="i in data.interactions" :key="i.id">
            <div class="text-xs text-ink3">{{ longDay(i.day) }}<template v-if="i.channel"> · {{ i.channel }}</template></div>
            <div v-if="i.note">{{ i.note }}</div>
          </li>
        </ol>
        <p v-else class="text-sm text-ink3">Chưa ghi lần nào.</p>
      </Section>
    </div>
  </template>
</template>
