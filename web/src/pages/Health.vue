<script setup lang="ts">
import { ref } from 'vue'
import { ArrowDown, ArrowUp } from '@lucide/vue'
import { useRead } from '../data'
import { num } from '../format'
import AskChip from '../components/AskChip.vue'
import Empty from '../components/Empty.vue'
import Line from '../components/Line.vue'
import PageHead from '../components/PageHead.vue'

const days = ref(30)
const { data } = useRead('metric_series', () => ({ days: days.value }))
</script>

<template>
  <PageHead title="Sức khoẻ" sub="Mỗi thứ đo được là một chỉ số; kể cho agent là nó ghi.">
    <AskChip size="sm" words="Hôm nay mình nặng " label="Cân nặng" />
    <AskChip size="sm" words="Tối qua ngủ " label="Giấc ngủ" />
    <AskChip size="sm" words="Tâm trạng hôm nay (1-10): " label="Tâm trạng" />
  </PageHead>
  <div class="mb-4 flex gap-1 text-xs" role="group" aria-label="Khoảng thời gian">
    <button
      v-for="n in [7, 30, 90, 365]"
      :key="n"
      type="button"
      class="rounded-full border px-2.5 py-0.5"
      :class="days === n ? 'border-accent text-accent' : 'border-line text-ink2'"
      @click="days = n"
    >
      {{ n === 365 ? '1 năm' : `${n} ngày` }}
    </button>
  </div>
  <div v-if="data" class="grid gap-4 md:grid-cols-2">
    <section v-for="m in data" :key="m.name" class="rounded-2xl border border-line bg-panel p-5">
      <div class="mb-2 flex items-baseline gap-2">
        <h2 class="flex-1 font-semibold first-letter:uppercase">{{ m.name }}</h2>
        <span class="text-xl font-semibold tabular-nums">{{ m.latest !== null ? num(m.latest) : '–' }}</span>
        <span class="text-xs text-ink3">{{ m.unit }}</span>
        <span v-if="m.latest !== null && m.previous !== null && m.latest !== m.previous" class="inline-flex items-center text-xs text-ink2">
          <component :is="m.latest > m.previous ? ArrowUp : ArrowDown" :size="12" />{{ num(Math.abs(m.latest - m.previous)) }}
        </span>
      </div>
      <Line :points="m.points" :unit="m.unit" />
    </section>
    <Empty v-if="!data.length" class="rounded-2xl border border-line bg-panel p-5 md:col-span-2" text="Chưa có chỉ số nào trong khoảng này." />
  </div>
</template>
