<script setup lang="ts">
import { computed } from 'vue'
import { ArrowUpRight, CalendarDays, HeartPulse, Inbox, ListChecks, Loader2, NotebookPen, Plus, Repeat, Sparkles, StickyNote, Sunrise, Target, Users, Wallet } from '@lucide/vue'
import { useRead } from '../data'
import { BRIEF_ASK, busy, conversation, ASSISTANT, say, writeBrief } from '../assistant'
import { compact, greeting, longDay, money, num, shortDay, todayIso } from '../format'
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
import { Button } from '../components/ui/button'
import { Card, CardContent } from '../components/ui/card'

const { data: d, error, loading } = useRead('dashboard')

const todayTasks = computed(() => (d.value ? [...d.value.tasks.overdue, ...d.value.tasks.today] : []))
const briefIsToday = computed(() => !!d.value?.brief && d.value.brief.summary.path.includes(d.value.date))
const primary = computed(() => {
  const m = d.value?.money
  return m?.currencies.find((c) => c.currency === m.primary_currency) ?? m?.currencies[0]
})
const spendBars = computed(() => (d.value?.money.daily_spending ?? []).map((p) => ({ label: shortDay(p.day), value: p.amount })))
const metrics = computed(() => {
  const dashboard = d.value
  if (!dashboard) return []
  const overdue = dashboard.tasks.overdue.length
  const tracked = dashboard.habits.length
  const done = dashboard.habits.filter((habit) => habit.done_today).length
  return [
    {
      title: 'Việc cần làm', to: '/tasks', icon: ListChecks, tone: 'focus', value: num(todayTasks.value.length),
      detail: overdue ? num(overdue) + ' việc quá hạn' : num(dashboard.tasks.done_today.length) + ' đã xong hôm nay',
      urgent: overdue > 0,
    },
    {
      title: 'Thói quen hôm nay', to: '/habits', icon: Repeat, tone: 'routine', value: tracked ? num(done) + '/' + num(tracked) : '—',
      detail: tracked ? 'Đã ghi nhận' : 'Chưa theo dõi thói quen',
    },
    {
      title: 'Chi tiêu tháng này', to: '/money', icon: Wallet, tone: 'money',
      value: primary.value ? money(primary.value.spending, primary.value.currency) : '—',
      detail: primary.value ? 'Tiền tệ chính · ' + primary.value.currency : 'Chưa có giao dịch',
    },
    {
      title: 'Inbox', to: '/inbox', icon: Inbox, tone: 'capture', value: num(dashboard.inbox.length),
      detail: dashboard.inbox.length ? 'Mục đang chờ xử lý' : 'Inbox trống',
    },
  ]
})
</script>

<template>
  <div class="@container overview space-y-6">
    <header class="overview-hero relative isolate flex min-h-[248px] flex-col justify-end overflow-hidden rounded-[1.4rem] px-5 pb-6 pt-16 text-white @min-[34rem]:min-h-[290px] @min-[34rem]:px-8 @min-[34rem]:pb-8">
      <p class="mb-3 flex items-center gap-2 text-xs font-semibold text-white/85">
        <CalendarDays class="size-4" /> {{ longDay(d?.date ?? todayIso()) }}
      </p>
      <h1 class="font-display text-[clamp(2.5rem,7cqi,4.65rem)] leading-[.98] tracking-[-.04em]">{{ greeting() }}</h1>
      <div class="mt-5 flex flex-wrap items-center gap-2">
        <Button type="button" class="rounded-full bg-[#e4edca] px-4 text-[#23341c] hover:bg-white" @click="say('Ghi vào inbox: ')">
          <Plus /> Ghi nhanh
        </Button>
        <AskChip class="h-9 rounded-full border-white/40 bg-black/25 px-3 text-white hover:bg-black/45 hover:text-white" words="Hôm nay mình nên tập trung vào gì?" label="Lên kế hoạch" now />
        <AskChip class="h-9 rounded-full border-white/40 bg-black/25 px-3 text-white hover:bg-black/45 hover:text-white" words="Nhìn lại tuần này và viết review tuần giúp mình." label="Review tuần" />
      </div>
    </header>

    <p v-if="error" role="alert" class="rounded-xl border border-critical/30 bg-critical-wash px-4 py-3 text-sm">
      Không tải được tổng quan. {{ error }}
    </p>
    <p v-if="!d && loading" role="status" class="flex items-center gap-2 text-sm text-ink2">
      <Loader2 class="size-4 motion-safe:animate-spin" /> Đang tải tổng quan…
    </p>

    <template v-if="d">
      <div class="overview-metrics grid grid-cols-2 overflow-hidden rounded-2xl border border-line bg-panel @min-[46rem]:grid-cols-4" aria-label="Tóm tắt hôm nay">
        <RouterLink v-for="metric in metrics" :key="metric.to" :to="metric.to" :data-tone="metric.tone"
          class="metric-cell min-w-0 p-4 transition-colors hover:bg-panel2 focus-visible:z-10 focus-visible:outline-2 focus-visible:-outline-offset-2 focus-visible:outline-ring @min-[54rem]:p-5">
          <div class="flex items-center justify-between gap-2">
            <span class="metric-icon grid size-8 shrink-0 place-items-center rounded-lg">
              <component :is="metric.icon" class="size-4" />
            </span>
            <ArrowUpRight class="size-4 text-ink3" aria-hidden="true" />
          </div>
          <div class="mt-2.5 text-xs font-semibold text-ink2">{{ metric.title }}</div>
          <div class="mt-1 break-words text-[clamp(1.25rem,3cqi,1.9rem)] leading-none font-semibold tracking-tight tabular-nums">{{ metric.value }}</div>
          <div class="mt-1.5 text-xs" :class="metric.urgent ? 'font-semibold text-critical' : 'text-ink2'">{{ metric.detail }}</div>
        </RouterLink>
      </div>

      <div class="grid gap-4 @min-[47rem]:grid-cols-[minmax(0,1.45fr)_minmax(0,1fr)]">
        <Section :icon="ListChecks" title="Việc hôm nay" to="/tasks" :count="todayTasks.length">
          <template v-if="todayTasks.length || d.tasks.done_today.length" #actions><AskChip words="Thêm việc: " label="Thêm" /></template>
          <ul v-if="todayTasks.length || d.tasks.done_today.length">
            <TaskRow v-for="t in todayTasks" :key="t.id" :task="t" />
            <TaskRow v-for="t in d.tasks.done_today" :key="t.id" :task="t" done />
          </ul>
          <div v-else class="rounded-xl border border-dashed border-line bg-bg p-4">
            <p class="text-sm font-semibold">Hôm nay chưa có việc nào</p>
            <p class="mt-1 text-xs text-ink2">Thêm một việc hoặc nhờ Assistant chọn ưu tiên.</p>
            <Button type="button" variant="outline" size="sm" class="mt-3 rounded-full" @click="say('Thêm việc: ')">
              <Plus /> Thêm việc
            </Button>
          </div>
          <p v-if="d.tasks.upcoming.length || d.tasks.anytime.length" class="mt-2 text-xs text-ink3">{{ d.tasks.upcoming.length }} việc trong 7 ngày tới · {{ d.tasks.anytime.length }} việc chưa xếp lịch</p>
        </Section>

        <Card class="brief-card min-w-0 gap-0 rounded-2xl border-line py-5 shadow-none">
          <CardContent class="px-5">
            <div class="flex flex-wrap items-center gap-2.5">
              <span class="grid size-9 shrink-0 place-items-center rounded-xl bg-violet-wash text-violet-text">
                <Sunrise class="size-4" />
              </span>
              <h2 class="text-base font-semibold">Bản tin sáng</h2>
              <span v-if="d.brief && !briefIsToday" class="text-xs text-ink2">Bản tin trước</span>
            </div>
            <Markdown v-if="d.brief" :source="d.brief.body" class="mt-3 text-sm" />
            <p v-else class="mt-3 text-sm leading-relaxed text-ink2">Nhờ Assistant tóm tắt lịch, việc quá hạn và các ưu tiên hôm nay.</p>
            <Button type="button" variant="outline" size="sm" class="mt-4 rounded-full" :disabled="busy()" @click="writeBrief">
              <Loader2 v-if="conversation(ASSISTANT).pending?.ask === BRIEF_ASK" class="motion-safe:animate-spin" />
              <Sparkles v-else />
              {{ briefIsToday ? 'Viết lại bản tin' : 'Viết bản tin' }}
            </Button>
          </CardContent>
        </Card>
      </div>

      <div class="grid gap-4 @min-[35rem]:grid-cols-2 @min-[63rem]:grid-cols-3" aria-label="Các lĩnh vực">
        <Section :icon="Repeat" title="Thói quen" to="/habits">
          <template #actions><AskChip words="Mình muốn bắt đầu thói quen " label="Thêm" /></template>
          <ul v-if="d.habits.length">
            <HabitRow v-for="h in d.habits" :key="h.habit" :habit="h" :today="d.date" :weeks="6" />
          </ul>
          <Empty v-else text="Chưa theo dõi thói quen nào." words="Mình muốn bắt đầu thói quen " label="Bắt đầu một thói quen" />
        </Section>

        <Section :icon="Target" title="Mục tiêu" to="/goals" :count="d.goals.length">
          <ul v-if="d.goals.length" class="divide-y divide-line">
            <GoalCard v-for="g in d.goals" :key="g.summary.id" :goal="g" />
          </ul>
          <Empty v-else text="Chưa có mục tiêu nào." words="Giúp mình đặt mục tiêu cho năm nay." label="Đặt mục tiêu" />
        </Section>

        <Section :icon="Users" title="Mối quan hệ" to="/people">
          <ul v-if="d.people_due.length || d.birthdays.length">
            <PersonRow v-for="p in d.people_due.slice(0, 5)" :key="'c' + p.id" :person="p" />
            <PersonRow v-for="p in d.birthdays" :key="'b' + p.id" :person="p" show="birthday" />
          </ul>
          <Empty v-else text="Không ai cần liên lạc gấp, không sinh nhật nào trong 30 ngày." words="Mình muốn lifeos nhắc giữ liên lạc với " label="Thêm người quan trọng" />
        </Section>

        <Section :icon="Wallet" title="Tài chính tháng này" to="/money">
          <template #actions><AskChip words="Vừa chi " label="Ghi chi tiêu" /></template>
          <template v-if="primary">
            <div class="mb-3 flex flex-wrap gap-x-6 gap-y-2">
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

        <Section :icon="HeartPulse" title="Sức khoẻ" to="/health">
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

        <Section :icon="NotebookPen" title="Nhật ký" to="/journal">
          <template #actions><AskChip :words="d.journal_written_today ? 'Ghi thêm vào nhật ký hôm nay: ' : 'Viết nhật ký hôm nay: '" label="Viết" /></template>
          <ul v-if="d.journal.length" class="space-y-2">
            <li v-for="j in d.journal.slice(0, 3)" :key="j.day">
              <RouterLink :to="`/doc/${j.day}`" class="text-xs font-medium text-ink2 hover:underline">{{ longDay(j.day) }}</RouterLink>
              <p class="line-clamp-2 text-sm">{{ j.excerpt }}</p>
            </li>
          </ul>
          <Empty v-else text="Chưa có trang nhật ký nào." words="Viết nhật ký hôm nay: " label="Viết trang đầu tiên" />
        </Section>

        <Section :icon="Inbox" title="Inbox" to="/inbox" :count="d.inbox.length">
          <ul v-if="d.inbox.length" class="space-y-1 text-sm">
            <li v-for="i in d.inbox.slice(0, 5)" :key="i.id" class="truncate">· {{ i.text }}</li>
          </ul>
          <Empty v-else text="Inbox trống." />
          <div class="mt-2 flex gap-1.5">
            <AskChip words="Ghi vào inbox: " label="Ghi nhanh" />
            <AskChip v-if="d.inbox.length" words="Phân loại inbox giúp mình." label="Phân loại" now />
          </div>
        </Section>

        <Section :icon="StickyNote" title="Ghi chú gần đây" to="/notes">
          <ul v-if="d.notes.length" class="space-y-1 text-sm">
            <li v-for="n in d.notes" :key="n.id"><RouterLink :to="`/doc/${n.id}`" class="hover:underline">{{ n.title }}</RouterLink></li>
          </ul>
          <Empty v-else text="Chưa có ghi chú." words="Lưu ghi chú: " label="Lưu một ý tưởng" />
        </Section>
      </div>
    </template>
  </div>
</template>
