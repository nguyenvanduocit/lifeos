<script setup lang="ts">
// The crew: agents the user asked for. Nothing is edited here: the user
// tells the Assistant, and the list changes as it saves.
import { computed } from 'vue'
import { MessageCircle } from '@lucide/vue'
import { useRead } from '../data'
import { conversation, panel, talkTo } from '../assistant'
import AgentBadge from '../components/chat/AgentBadge.vue'
import AskChip from '../components/AskChip.vue'
import Empty from '../components/Empty.vue'
import PageHead from '../components/PageHead.vue'

const { data } = useRead('list_crew')
const active = computed(() => data.value?.filter((m) => m.status === 'active') ?? [])
const retired = computed(() => data.value?.filter((m) => m.status !== 'active') ?? [])

function lastWords(id: string): string {
  const turns = conversation(id).turns
  const t = turns[turns.length - 1]
  return t ? `${t.at.slice(5)} · ${t.ask}` : ''
}
</script>

<template>
  <PageHead title="Crew" sub="Những agent riêng cho từng phần cuộc sống: mỗi agent có vai trò, hướng dẫn, tool, memory và cuộc trò chuyện riêng.">
    <AskChip words="Mình muốn có một agent riêng để " label="Tuyển thành viên" />
    <AskChip words="Gợi ý crew phù hợp với mình dựa trên mục tiêu và thói quen hiện tại." label="Gợi ý crew" now />
  </PageHead>

  <div v-if="data" class="grid gap-4 md:grid-cols-2 xl:grid-cols-3">
    <article
      v-for="m in active"
      :key="m.id"
      class="flex flex-col rounded-xl border bg-panel p-4"
      :class="panel.open && panel.agent === m.id ? 'border-violet' : 'border-line'"
    >
      <header class="flex items-center gap-3">
        <AgentBadge :id="m.id" :name="m.name" :size="34" />
        <div class="min-w-0">
          <h2 class="truncate font-semibold">{{ m.name }}</h2>
          <p class="truncate text-xs text-ink2">{{ m.role }}</p>
        </div>
        <span v-if="conversation(m.id).pending" class="ml-auto text-xs text-violet-text">đang làm…</span>
      </header>
      <p class="mt-3 line-clamp-3 text-sm text-ink2">{{ m.instructions }}</p>
      <div class="mt-3 flex flex-wrap gap-x-3 gap-y-1 text-[11px] text-ink3">
        <span>{{ m.memories }} memory</span>
        <span>{{ m.tools.length ? `${m.tools.length} tool` : 'mọi tool' }}</span>
        <span v-if="m.model">{{ m.model }}</span>
        <span v-if="m.effort">effort {{ m.effort }}</span>
      </div>
      <p v-if="lastWords(m.id)" class="mt-2 truncate text-xs text-ink3">{{ lastWords(m.id) }}</p>
      <div class="mt-auto flex flex-wrap gap-1.5 pt-3">
        <button
          type="button"
          class="inline-flex items-center gap-1 rounded-full bg-violet px-3 py-1 text-xs text-white"
          @click="talkTo(m.id)"
        >
          <MessageCircle :size="12" /> Trò chuyện
        </button>
        <AskChip :words="`Đổi hướng dẫn của ${m.name} (id ${m.id}): `" label="Đổi" />
        <AskChip :words="`Cho ${m.name} (id ${m.id}) nghỉ.`" label="Cho nghỉ" />
      </div>
    </article>
    <Empty
      v-if="!active.length"
      text="Chưa có thành viên nào. Ví dụ: một huấn luyện viên sức khoẻ theo dõi giấc ngủ và chạy bộ, một cố vấn tài chính xem chi tiêu mỗi tuần."
      words="Mình muốn có một huấn luyện viên sức khoẻ riêng, theo dõi giấc ngủ, cân nặng và chạy bộ."
      label="Tuyển huấn luyện viên sức khoẻ"
    />
  </div>

  <section v-if="retired.length" class="mt-6">
    <h2 class="mb-2 text-sm font-semibold text-ink2">Đã nghỉ</h2>
    <ul class="flex flex-wrap gap-2">
      <li v-for="m in retired" :key="m.id" class="flex items-center gap-2 rounded-lg border border-line bg-panel px-3 py-1.5 text-sm opacity-70">
        <AgentBadge :id="m.id" :name="m.name" :size="18" /> {{ m.name }}
        <AskChip :words="`Gọi ${m.name} (id ${m.id}) trở lại.`" label="Gọi lại" />
      </li>
    </ul>
  </section>
</template>
