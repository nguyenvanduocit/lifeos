<script setup lang="ts">
import { computed } from 'vue'
import { Loader2, Sunrise } from '@lucide/vue'
import { useRead } from '../data'
import { BRIEF_ASK, busy, conversation, ASSISTANT, writeBrief } from '../assistant'
import { compact, greeting, longDay, money, num, shortDay } from '../format'
import AskChip from '../components/AskChip.vue'
import Bars from '../components/Bars.vue'
import BudgetRow from '../components/BudgetRow.vue'
import Empty from '../components/Empty.vue'
import GoalCard from '../components/GoalCard.vue'
import HabitRow from '../components/HabitRow.vue'
import Line from '../components/Line.vue'
import Markdown from '../components/Markdown.vue'
import PersonRow from '../components/PersonRow.vue'
import Section from '../components/Section.vue'
import TaskRow from '../components/TaskRow.vue'

const { data: d, error } = useRead('dashboard')

const todayTasks = computed(() => (d.value ? [...d.value.tasks.overdue, ...d.value.tasks.today] : []))
const briefIsToday = computed(() => !!d.value?.brief && d.value.brief.summary.path.includes(d.value.date))
const primary = computed(() => {
  const m = d.value?.money
  return m?.currencies.find((c) => c.currency === m.primary_currency) ?? m?.currencies[0]
})
const spendBars = computed(() => (d.value?.money.daily_spending ?? []).map((p) => ({ label: shortDay(p.day), value: p.amount })))
</script>

<template>
  <p v-if="error" class="text-sm text-critical">{{ error }}</p>
  <div v-if="d" class="space-y-4">
    <header class="flex flex-wrap items-end gap-3">
      <div>
        <p class="text-sm text-ink2">{{ longDay(d.date) }}</p>
        <h1 class="text-2xl font-semibold tracking-tight">{{ greeting() }}</h1>
      </div>
      <div class="ml-auto flex flex-wrap gap-1.5">
        <AskChip words="Hôm nay mình nên tập trung vào gì?" label="Nên làm gì hôm nay?" now />
        <AskChip words="Nhìn lại tuần này và viết review tuần giúp mình." label="Review tuần" />
      </div>
    </header>

    <!-- The morning brief the agent wrote -->
    <section class="rounded-xl border border-line bg-panel p-4">
      <div class="mb-2 flex items-center gap-2">
        <Sunrise :size="16" class="text-ink2" />
        <h2 class="text-sm font-semibold">Bản tin sáng</h2>
        <span v-if="d.brief && !briefIsToday" class="text-xs text-ink3">(của hôm qua)</span>
        <button
          type="button"
          class="ml-auto inline-flex items-center gap-1 rounded-full border border-line px-2 py-0.5 text-xs text-ink2 hover:border-accent hover:text-accent disabled:opacity-50"
          :disabled="busy()"
          @click="writeBrief"
        >
          <Loader2 v-if="conversation(ASSISTANT).pending?.ask === BRIEF_ASK" :size="12" class="animate-spin" />
          {{ briefIsToday ? 'Viết lại' : 'Nhờ agent viết' }}
        </button>
      </div>
      <Markdown v-if="d.brief" :source="d.brief.body" class="text-sm" />
      <p v-else class="text-sm text-ink3">Chưa có bản tin. Agent đọc toàn bộ dashboard rồi tóm lại những gì quan trọng hôm nay.</p>
    </section>

    <div class="grid gap-4 md:grid-cols-2 xl:grid-cols-3">
      <Section title="Việc hôm nay" to="/tasks" :count="todayTasks.length">
        <template #actions><AskChip words="Thêm việc: " label="Thêm" /></template>
        <ul v-if="todayTasks.length || d.tasks.done_today.length">
          <TaskRow v-for="t in todayTasks" :key="t.id" :task="t" />
          <TaskRow v-for="t in d.tasks.done_today" :key="t.id" :task="t" done />
        </ul>
        <Empty v-else text="Không có việc nào cho hôm nay." words="Giúp mình chọn 3 việc quan trọng cho hôm nay." label="Chọn việc cho hôm nay" />
        <p v-if="d.tasks.upcoming.length" class="mt-2 text-xs text-ink3">{{ d.tasks.upcoming.length }} việc trong 7 ngày tới · {{ d.tasks.anytime.length }} việc chưa xếp lịch</p>
      </Section>

      <Section title="Thói quen" to="/habits">
        <template #actions><AskChip words="Mình muốn bắt đầu thói quen " label="Thêm" /></template>
        <ul v-if="d.habits.length">
          <HabitRow v-for="h in d.habits" :key="h.habit" :habit="h" :today="d.date" :weeks="6" />
        </ul>
        <Empty v-else text="Chưa theo dõi thói quen nào." words="Mình muốn bắt đầu thói quen " label="Bắt đầu một thói quen" />
      </Section>

      <Section title="Mục tiêu" to="/goals" :count="d.goals.length">
        <ul v-if="d.goals.length" class="divide-y divide-line">
          <GoalCard v-for="g in d.goals" :key="g.summary.id" :goal="g" />
        </ul>
        <Empty v-else text="Chưa có mục tiêu nào." words="Giúp mình đặt mục tiêu cho năm nay." label="Đặt mục tiêu" />
      </Section>

      <Section title="Mối quan hệ" to="/people">
        <ul v-if="d.people_due.length || d.birthdays.length">
          <PersonRow v-for="p in d.people_due.slice(0, 5)" :key="'c' + p.id" :person="p" />
          <PersonRow v-for="p in d.birthdays" :key="'b' + p.id" :person="p" show="birthday" />
        </ul>
        <Empty v-else text="Không ai cần liên lạc gấp, không sinh nhật nào trong 30 ngày." words="Mình muốn lifeos nhắc giữ liên lạc với " label="Thêm người quan trọng" />
      </Section>

      <Section title="Tài chính tháng này" to="/money">
        <template #actions><AskChip words="Vừa chi " label="Ghi chi tiêu" /></template>
        <template v-if="primary">
          <div class="mb-3 flex gap-6">
            <div>
              <div class="text-xs text-ink3">Đã chi</div>
              <div class="text-lg font-semibold tabular-nums">{{ money(primary.spending, primary.currency) }}</div>
            </div>
            <div>
              <div class="text-xs text-ink3">Thu</div>
              <div class="text-lg font-semibold tabular-nums">{{ money(primary.income, primary.currency) }}</div>
            </div>
          </div>
          <Bars v-if="spendBars.length" :points="spendBars" :format="(v) => compact(v)" :height="70" />
          <ul v-if="d.money.budgets.length" class="mt-2">
            <BudgetRow v-for="b in d.money.budgets.slice(0, 4)" :key="b.category + b.currency" :line="b" />
          </ul>
        </template>
        <Empty v-else text="Tháng này chưa ghi khoản nào." words="Vừa chi " label="Ghi khoản chi đầu tiên" />
      </Section>

      <Section title="Sức khoẻ" to="/health">
        <template #actions><AskChip words="Hôm nay mình " label="Ghi chỉ số" /></template>
        <ul v-if="d.health.length" class="space-y-2">
          <li v-for="m in d.health.slice(0, 4)" :key="m.name" class="flex items-center gap-3">
            <div class="w-28 shrink-0">
              <div class="truncate text-xs first-letter:uppercase text-ink2">{{ m.name }}</div>
              <div class="text-sm font-semibold tabular-nums">{{ m.latest !== null ? num(m.latest) : '–' }} <span class="text-xs font-normal text-ink3">{{ m.unit }}</span></div>
            </div>
            <Line class="flex-1" :points="m.points" :unit="m.unit" spark />
          </li>
        </ul>
        <Empty v-else text="Chưa có chỉ số nào trong 30 ngày qua." words="Hôm nay mình nặng " label="Ghi cân nặng, giấc ngủ…" />
      </Section>

      <Section title="Nhật ký" to="/journal">
        <template #actions><AskChip :words="d.journal_written_today ? 'Ghi thêm vào nhật ký hôm nay: ' : 'Viết nhật ký hôm nay: '" label="Viết" /></template>
        <ul v-if="d.journal.length" class="space-y-2">
          <li v-for="j in d.journal.slice(0, 3)" :key="j.day">
            <RouterLink :to="`/doc/${j.day}`" class="text-xs font-medium text-ink2 hover:underline">{{ longDay(j.day) }}</RouterLink>
            <p class="line-clamp-2 text-sm">{{ j.excerpt }}</p>
          </li>
        </ul>
        <Empty v-else text="Chưa có trang nhật ký nào." words="Viết nhật ký hôm nay: " label="Viết trang đầu tiên" />
      </Section>

      <Section title="Inbox" to="/inbox" :count="d.inbox.length">
        <ul v-if="d.inbox.length" class="space-y-1 text-sm">
          <li v-for="i in d.inbox.slice(0, 5)" :key="i.id" class="truncate">· {{ i.text }}</li>
        </ul>
        <Empty v-else text="Inbox trống." />
        <div class="mt-2 flex gap-1.5">
          <AskChip words="Ghi vào inbox: " label="Ghi nhanh" />
          <AskChip v-if="d.inbox.length" words="Phân loại inbox giúp mình." label="Phân loại" now />
        </div>
      </Section>

      <Section title="Ghi chú gần đây" to="/notes">
        <ul v-if="d.notes.length" class="space-y-1 text-sm">
          <li v-for="n in d.notes" :key="n.id"><RouterLink :to="`/doc/${n.id}`" class="hover:underline">{{ n.title }}</RouterLink></li>
        </ul>
        <Empty v-else text="Chưa có ghi chú." words="Lưu ghi chú: " label="Lưu một ý tưởng" />
      </Section>
    </div>
  </div>
</template>
