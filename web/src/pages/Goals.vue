<script setup lang="ts">
import { computed } from 'vue'
import { useRead } from '../data'
import AskChip from '../components/AskChip.vue'
import Empty from '../components/Empty.vue'
import GoalCard from '../components/GoalCard.vue'
import PageHead from '../components/PageHead.vue'
import Section from '../components/Section.vue'

const { data } = useRead('list_goals')
const active = computed(() => data.value?.filter((g) => g.summary.status === 'active') ?? [])
const rest = computed(() => data.value?.filter((g) => g.summary.status !== 'active') ?? [])
</script>

<template>
  <PageHead title="Mục tiêu" sub="Mỗi mục tiêu có lý do, tiến độ và các việc phục vụ nó.">
    <AskChip size="sm" primary words="Mình muốn đặt mục tiêu: " label="Mục tiêu mới" />
    <AskChip size="sm" words="Xem lại các mục tiêu và cập nhật tiến độ giúp mình." label="Rà soát" now />
  </PageHead>
  <div v-if="data" class="grid gap-4 md:grid-cols-2">
    <Section title="Đang theo đuổi" :count="active.length" class="md:col-span-2">
      <ul v-if="active.length" class="grid gap-x-8 divide-line md:grid-cols-2">
        <GoalCard v-for="g in active" :key="g.summary.id" :goal="g" full />
      </ul>
      <Empty v-else text="Chưa có mục tiêu đang theo đuổi." words="Giúp mình đặt 3 mục tiêu cho năm tới." label="Đặt mục tiêu" />
    </Section>
    <Section v-if="rest.length" title="Tạm dừng, đã xong, đã bỏ" class="md:col-span-2">
      <ul class="grid gap-x-8 md:grid-cols-2"><GoalCard v-for="g in rest" :key="g.summary.id" :goal="g" /></ul>
    </Section>
  </div>
</template>
