<script setup lang="ts">
import { Check, Flame } from '@lucide/vue'
import { useRead } from '../data'
import { ask, busy } from '../assistant'
import { todayIso } from '../format'
import AskChip from '../components/AskChip.vue'
import Empty from '../components/Empty.vue'
import Heatmap from '../components/Heatmap.vue'
import PageHead from '../components/PageHead.vue'

const { data } = useRead('habits')
const today = todayIso()
</script>

<template>
  <PageHead title="Thói quen" sub="12 tuần gần nhất, mỗi ô một ngày; đậm hơn là làm nhiều hơn.">
    <AskChip words="Mình muốn bắt đầu thói quen " label="Thói quen mới" />
    <AskChip words="Nhận xét các thói quen của mình 4 tuần qua." label="Nhận xét" now />
  </PageHead>
  <div v-if="data" class="grid gap-4 md:grid-cols-2">
    <section v-for="h in data" :key="h.habit" class="rounded-xl border border-line bg-panel p-4">
      <div class="mb-3 flex items-center gap-3">
        <div class="flex-1">
          <h2 class="font-semibold first-letter:uppercase">{{ h.habit }}</h2>
          <div class="flex gap-3 text-xs text-ink2">
            <span class="inline-flex items-center gap-0.5"><Flame :size="12" /> chuỗi {{ h.streak }} ngày</span>
            <span>{{ h.done_this_week }}/{{ h.target_per_week }} ngày tuần này</span>
            <span v-if="h.unit">đơn vị: {{ h.unit }}</span>
          </div>
        </div>
        <button
          type="button"
          class="inline-flex items-center gap-1 rounded-full border px-2.5 py-1 text-xs"
          :class="h.done_today ? 'border-good text-good' : 'border-line text-ink2 hover:border-accent hover:text-accent'"
          :disabled="h.done_today || busy()"
          @click="ask(`Ghi thói quen “${h.habit}” hôm nay.`)"
        >
          <Check :size="13" /> {{ h.done_today ? 'Hôm nay đã làm' : 'Ghi hôm nay' }}
        </button>
      </div>
      <Heatmap :values="h.recent" :end="today" :unit="h.unit" />
      <div class="mt-2 flex gap-1.5">
        <AskChip :words="`Đổi mục tiêu thói quen “${h.habit}” thành `" label="Đổi mục tiêu" />
        <AskChip :words="`Dừng theo dõi thói quen “${h.habit}”.`" label="Dừng" />
      </div>
    </section>
    <Empty v-if="!data.length" text="Chưa theo dõi thói quen nào." words="Mình muốn bắt đầu thói quen " label="Bắt đầu" />
  </div>
</template>
