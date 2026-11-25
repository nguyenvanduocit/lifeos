<script setup lang="ts">
// Today, drawn as Relay draws its home: the day and week, a greeting, a strip of figures that each carry their frame,
// then the day's tasks beside the morning brief, the week beside the habits, and the life areas that need a look.
import { computed, ref, type Component } from 'vue'
import {
  Cake, CalendarDays, Check, CircleCheck, FileText, Flame, HeartPulse, Inbox, ListChecks, Loader2, MessageCircle, PenLine, Plus, Repeat, RefreshCw, Sparkles, Target, Timer, Users, Wallet,
} from '@lucide/vue'
import { useRead } from '../data'
import { ask, briefing, busy, writeBrief } from '../assistant'
import { greeting, headDay, isoDay, isoWeek, moneyShort, num, parseDay, shortDay, todayIso } from '../format'
import LoadState from '../components/LoadState.vue'
import Markdown from '../components/Markdown.vue'
import AskChip from '../components/AskChip.vue'
import Avatar from '../components/Avatar.vue'
import PageHeader from '../components/ds/PageHeader.vue'
import StatStrip from '../components/ds/StatStrip.vue'
import Panel from '../components/ds/Panel.vue'
import { plugins } from '../plugins'
import PluginFrame from '../components/PluginFrame.vue'
import WidgetSlot from '../components/WidgetSlot.vue'
import TableHead from '../components/ds/TableHead.vue'
import StatusTag from '../components/ds/StatusTag.vue'
import TaskLine from '../components/tasks/TaskLine.vue'
import { dayOf, whenOf } from '../components/tasks/when'
import { Button } from '../components/ui/button'

const { data: d, error, loading, retry } = useRead('dashboard')
const today = computed(() => d.value?.date ?? todayIso())

const goalTitle = computed(() => new Map((d.value?.goals ?? []).map((g) => [g.summary.id, g.summary.title])))
const dueToday = computed(() => (d.value ? [...d.value.tasks.overdue, ...d.value.tasks.today] : []))
const briefIsToday = computed(() => !!d.value?.brief && d.value.brief.summary.path.includes(today.value))
const primary = computed(() => {
  const m = d.value?.money
  return m?.currencies.find((c) => c.currency === m.primary_currency) ?? m?.currencies[0]
})
const budgetTotal = computed(() => {
  const m = d.value?.money
  if (!m || !primary.value) return null
  const lines = m.budgets.filter((b) => b.currency === primary.value?.currency)
  return lines.length ? { budget: lines.reduce((a, b) => a + b.budget, 0), spent: lines.reduce((a, b) => a + b.spent, 0) } : null
})
const daysLeftInMonth = computed(() => {
  const t = parseDay(today.value)
  return new Date(t.getFullYear(), t.getMonth() + 1, 0).getDate() - t.getDate()
})

const strip = computed(() => {
  const v = d.value
  if (!v) return []
  const done = v.tasks.done_today.length
  const habitsDone = v.habits.filter((h) => h.done_today).length
  const overdue = v.tasks.overdue.length
  const items: { key: string; icon: Component; to?: string; value: string; label: string; delta?: string; deltaTone?: 'good' | 'bad' | 'flat'; urgent?: boolean }[] = [
    { key: 'tasks', icon: CircleCheck, to: '/tasks', value: `${done}/${done + dueToday.value.length}`, label: 'tasks done today',
      delta: overdue ? `${overdue} late` : undefined, deltaTone: 'bad', urgent: overdue > 0 },
    { key: 'habits', icon: Repeat, to: '/habits', value: `${habitsDone}/${v.habits.length}`, label: 'habits logged' },
  ]
  if (primary.value) {
    items.push({ key: 'money', icon: Wallet, to: '/money', value: moneyShort(primary.value.spending, primary.value.currency),
      label: 'spent this month', delta: budgetTotal.value ? `${Math.round((budgetTotal.value.spent / budgetTotal.value.budget) * 100)}% of budgets` : undefined,
      deltaTone: budgetTotal.value && budgetTotal.value.spent > budgetTotal.value.budget ? 'bad' : 'flat' })
  }
  items.push({ key: 'inbox', icon: Inbox, to: '/inbox', value: String(v.inbox.length), label: 'waiting in inbox' })
  return items
})

// The week, Monday first; a day shows a dot when something is planned or due on it.
const week = computed(() => {
  const t = parseDay(today.value)
  const monday = new Date(t)
  monday.setDate(t.getDate() - ((t.getDay() + 6) % 7))
  return Array.from({ length: 7 }, (_, i) => {
    const day = new Date(monday)
    day.setDate(monday.getDate() + i)
    const iso = isoDay(day)
    return { iso, short: new Intl.DateTimeFormat('en-US', { weekday: 'short' }).format(day).slice(0, 2), n: day.getDate() }
  })
})
const allOpen = computed(() => (d.value ? [...d.value.tasks.overdue, ...d.value.tasks.today, ...d.value.tasks.upcoming] : []))
const picked = ref<string | null>(null)
const pickedDay = computed(() => picked.value ?? today.value)
const onDay = computed(() => allOpen.value.filter((t) => (pickedDay.value === today.value ? whenOf(t, today.value)?.tone === 'critical' || dayOf(t) === today.value : dayOf(t) === pickedDay.value)))
const busyDays = computed(() => new Set(allOpen.value.map(dayOf).filter(Boolean)))

const habitWeek = (recent: number[]) => recent.slice(-7)
// The Goal column shows only when one of today's tasks serves a goal.
const withGoal = computed(() => [...dueToday.value, ...(d.value?.tasks.done_today ?? [])].some((t) => t.goal))
const TASK_GRID = computed(() => withGoal.value ? 'minmax(0,2.2fr) minmax(0,1.2fr) minmax(0,0.9fr) minmax(0,0.6fr)' : 'minmax(0,3fr) minmax(0,1fr) minmax(0,0.6fr)')
const taskCols = computed(() => [
  { label: 'Task', icon: ListChecks },
  ...(withGoal.value ? [{ label: 'Goal', icon: Target, class: 'hidden @min-[44rem]:flex' }] : []),
  { label: 'When', icon: CalendarDays },
  { label: 'Priority', icon: Flame, class: 'hidden @min-[44rem]:flex' },
])
</script>

<template>
  <LoadState :loading="loading && !d" :error="error" label="today" class="mb-4" @retry="retry" />
  <template v-if="d">
    <PageHeader :title="greeting()">
      <template #eyebrow>
        <span>{{ headDay(today) }}</span><span class="text-ink3">•</span><span>Week {{ isoWeek(today) }}</span>
      </template>
      <AskChip size="sm" :icon="FileText" words="Review this week and write a weekly summary." label="Weekly review" />
      <AskChip size="sm" primary :icon="Plus" words="Capture in Inbox: " label="Capture" />
      <template #strip><StatStrip :items="strip" /></template>
    </PageHeader>

    <WidgetSlot class="mb-5" />

    <div class="grid gap-5 @min-[60rem]:grid-cols-[minmax(0,1.6fr)_minmax(0,1fr)]">
      <Panel title="Today’s tasks" :icon="ListChecks" :count="dueToday.length ? `${dueToday.length} open` : undefined" to="/tasks">
        <template #actions>
          <AskChip :icon="Plus" words="Add a task: " label="Add task" />
          <AskChip :icon="Sparkles" words="What should I focus on today?" label="Plan today" />
        </template>
        <div v-if="dueToday.length || d.tasks.done_today.length" role="table" aria-label="Today’s tasks">
          <TableHead :grid="TASK_GRID" :cols="taskCols" />
          <TaskLine v-for="t in dueToday" :key="t.id" :task="t" :goal="t.goal ? goalTitle.get(t.goal) : undefined" :show-goal="withGoal" :grid="TASK_GRID" :to="`/tasks/${t.id}`" />
          <TaskLine v-for="t in d.tasks.done_today" :key="t.id" :task="t" done :goal="t.goal ? goalTitle.get(t.goal) : undefined" :show-goal="withGoal" :grid="TASK_GRID" :to="`/tasks/${t.id}`" />
        </div>
        <div v-else class="flex items-center gap-4 border-t border-line px-5 py-6">
          <span class="grid size-10 place-items-center rounded-xl bg-panel2"><Check class="size-5 text-ink2" /></span>
          <div class="min-w-0 flex-1">
            <p class="font-medium">Nothing due today</p>
            <p class="text-[13.5px] text-ink2">{{ d.tasks.upcoming.length }} coming this week · {{ d.tasks.anytime.length }} without a date</p>
          </div>
        </div>
      </Panel>

      <Panel title="Morning brief" :icon="Sparkles" :count="d.brief ? (briefIsToday ? 'Today' : 'Earlier') : undefined" :count-tone="briefIsToday ? 'signal' : 'neutral'" pad>
        <Markdown v-if="d.brief" :source="d.brief.body" class="brief text-[14.5px] leading-relaxed" />
        <p v-else class="text-[14.5px] leading-relaxed text-ink2">No brief yet today. The Assistant can sum up your schedule, what is late and what matters most.</p>
        <div class="mt-5 flex flex-wrap items-center gap-2">
          <Button type="button" class="h-9 rounded-[10px] px-3.5" :disabled="busy()" @click="writeBrief">
            <Loader2 v-if="briefing" class="motion-safe:animate-spin" /><RefreshCw v-else />
            {{ briefIsToday ? 'Refresh' : 'Write brief' }}
          </Button>
          <AskChip v-if="d.brief" :icon="MessageCircle" words="About today’s brief: " label="Ask about it" />
        </div>
      </Panel>
    </div>

    <div class="mt-5 grid gap-5 @min-[60rem]:grid-cols-2">
      <Panel title="This week" :icon="CalendarDays" :count="`${d.tasks.upcoming.length + dueToday.length} planned`">
        <div class="grid grid-cols-7 border-y border-line px-3 py-3" role="radiogroup" aria-label="Day of the week">
          <button v-for="w in week" :key="w.iso" type="button" role="radio" :aria-checked="pickedDay === w.iso"
            class="mx-auto flex w-12 flex-col items-center gap-0.5 rounded-xl py-1.5 transition-colors hover:bg-panel2 focus-visible:outline-2 focus-visible:outline-ring"
            :class="w.iso === today ? 'bg-signal text-signal-fg hover:bg-signal' : pickedDay === w.iso ? 'bg-panel3' : ''"
            @click="picked = w.iso">
            <span class="text-[12.5px]" :class="w.iso === today ? 'text-signal-fg/75' : 'text-ink3'">{{ w.short }}</span>
            <span class="text-[17px] font-semibold tabular-nums">{{ w.n }}</span>
            <span class="size-1 rounded-full" :class="busyDays.has(w.iso) ? (w.iso === today ? 'bg-signal-fg' : 'bg-ink3') : 'bg-transparent'" />
          </button>
        </div>
        <ul v-if="onDay.length" class="divide-y divide-line">
          <li v-for="t in onDay" :key="t.id" class="relative flex items-center gap-3 px-5 py-3 hover:bg-panel2/70">
            <span class="w-[3px] self-stretch rounded-full" :class="whenOf(t, today)?.tone === 'critical' ? 'bg-critical' : t.priority === 'high' ? 'bg-ink' : 'bg-line2'" aria-hidden="true" />
            <div class="min-w-0 flex-1">
              <RouterLink :to="`/tasks/${t.id}`" class="block truncate text-[14.5px] font-medium after:absolute after:inset-0">{{ t.title }}</RouterLink>
              <p class="truncate text-[13px] text-ink2">
                <span :class="whenOf(t, today)?.tone === 'critical' ? 'text-critical' : ''">{{ whenOf(t, today)?.text }}</span>
                <template v-if="t.goal && goalTitle.get(t.goal)"> <span class="text-ink3">|</span> {{ goalTitle.get(t.goal) }}</template>
              </p>
            </div>
            <StatusTag v-if="t.priority === 'high'" tone="critical">High</StatusTag>
          </li>
        </ul>
        <p v-else class="px-5 py-6 text-[14px] text-ink2">Nothing planned for {{ pickedDay === today ? 'today' : shortDay(pickedDay) }}.</p>
      </Panel>

      <Panel title="Habits" :icon="Repeat" :count="d.habits.length ? `${d.habits.filter((h) => !h.done_today).length} left today` : undefined" :count-tone="d.habits.some((h) => !h.done_today) ? 'signal' : 'neutral'" to="/habits">
        <template #actions><AskChip :icon="Plus" words="I want to start tracking the habit " label="New habit" /></template>
        <ul v-if="d.habits.length" class="divide-y divide-line border-t border-line">
          <li v-for="h in d.habits" :key="h.habit" class="flex items-center gap-4 px-5 py-3">
            <div class="min-w-0 flex-1">
              <p class="truncate text-[14.5px] font-medium first-letter:uppercase">{{ h.habit }}</p>
              <p class="flex items-center gap-1 text-[13px] text-ink2">
                <Flame class="size-3.5" :class="h.streak >= 3 ? 'text-critical' : 'text-ink3'" aria-hidden="true" />{{ h.streak }}-day streak
                <span class="text-ink3">·</span>{{ h.done_this_week }}/{{ h.target_per_week }} this week
              </p>
            </div>
            <div class="hidden gap-1 sm:flex" :aria-label="`Last 7 days: ${habitWeek(h.recent).filter((v) => v > 0).length} done`">
              <span v-for="(v, i) in habitWeek(h.recent)" :key="i" class="size-3.5 rounded-[4px]" :class="v > 0 ? 'bg-ink/85' : 'bg-panel3'" />
            </div>
            <button type="button"
              class="inline-flex h-8 shrink-0 items-center gap-1.5 rounded-[10px] border px-3 text-[13px] font-medium transition-colors disabled:cursor-default"
              :class="h.done_today ? 'border-transparent bg-signal/40 text-signal-ink dark:bg-signal/15' : 'border-line2 hover:bg-panel2'"
              :disabled="h.done_today || busy()" @click="ask(`Log “${h.habit}” for today.`)">
              <Check class="size-3.5" />{{ h.done_today ? 'Done' : 'Log' }}
            </button>
          </li>
        </ul>
        <p v-else class="border-t border-line px-5 py-6 text-[14px] text-ink2">No habits yet.</p>
      </Panel>
    </div>

    <div class="mt-5 grid gap-5 @min-[48rem]:grid-cols-2 @min-[64rem]:grid-cols-3">
      <Panel title="Money" :icon="Wallet" to="/money" pad>
        <template v-if="primary">
          <p class="text-[30px] leading-none font-semibold tracking-[-0.02em] tabular-nums">{{ moneyShort(primary.spending, primary.currency) }}</p>
          <p class="mt-1.5 text-[13.5px] text-ink2">
            spent this month · {{ daysLeftInMonth }} days left
            <template v-if="budgetTotal"> · budgets <span class="tabular-nums" :class="budgetTotal.spent > budgetTotal.budget ? 'text-critical' : 'text-ink'">{{ moneyShort(budgetTotal.spent, primary.currency) }} / {{ moneyShort(budgetTotal.budget, primary.currency) }}</span></template>
          </p>
          <ul v-if="d.money.budgets.length" class="mt-4 space-y-2.5">
            <li v-for="b in [...d.money.budgets].sort((a, z) => z.spent / z.budget - a.spent / a.budget).slice(0, 4)" :key="b.category + b.currency" class="grid grid-cols-[6.5rem_minmax(0,1fr)_auto] items-center gap-3 text-[13.5px]">
              <span class="truncate first-letter:uppercase">{{ b.category }}</span>
              <span class="h-1.5 overflow-hidden rounded-full bg-panel3"><span class="block h-full rounded-full" :class="b.spent > b.budget ? 'bg-critical' : 'bg-ink'" :style="{ width: Math.min(100, (b.spent / b.budget) * 100) + '%' }" /></span>
              <span class="text-ink2 tabular-nums" :class="b.spent > b.budget ? 'text-critical' : ''">{{ Math.round((b.spent / b.budget) * 100) }}%</span>
            </li>
          </ul>
        </template>
        <p v-else class="text-[14px] text-ink2">No spending logged this month.</p>
        <AskChip class="mt-4" :icon="Plus" words="I just spent " label="Log expense" />
      </Panel>

      <Panel title="People" :icon="Users" to="/people" :count="d.people_due.length ? `${d.people_due.length} to reach out` : undefined">
        <ul v-if="d.people_due.length || d.birthdays.length" class="divide-y divide-line border-t border-line">
          <li v-for="p in d.birthdays.slice(0, 2)" :key="'b' + p.id" class="relative flex items-center gap-3 px-5 py-3 hover:bg-panel2/70">
            <Avatar :name="p.name" :size="32" />
            <div class="min-w-0 flex-1">
              <RouterLink :to="`/people/${p.id}`" class="block truncate text-[14.5px] font-medium after:absolute after:inset-0">{{ p.name }}</RouterLink>
              <p class="truncate text-[13px] text-ink2 first-letter:uppercase">{{ p.relation ?? 'Contact' }}</p>
            </div>
            <StatusTag tone="warning"><Cake class="-ml-0.5 size-3" />{{ p.days_to_birthday === 0 ? 'Birthday today' : `Birthday in ${p.days_to_birthday}d` }}</StatusTag>
          </li>
          <li v-for="p in d.people_due.filter((x) => !d!.birthdays.slice(0, 2).some((b) => b.id === x.id)).slice(0, 4)" :key="'c' + p.id" class="relative flex items-center gap-3 px-5 py-3 hover:bg-panel2/70">
            <Avatar :name="p.name" :size="32" />
            <div class="min-w-0 flex-1">
              <RouterLink :to="`/people/${p.id}`" class="block truncate text-[14.5px] font-medium after:absolute after:inset-0">{{ p.name }}</RouterLink>
              <p class="truncate text-[13px] text-ink2">{{ p.days_since_contact === null ? 'Never in touch' : `Last talked ${p.days_since_contact}d ago` }} · every {{ p.contact_every_days }}d</p>
            </div>
          </li>
        </ul>
        <p v-else class="border-t border-line px-5 py-6 text-[14px] text-ink2">Everyone is in touch. No birthdays in the next 30 days.</p>
      </Panel>

      <Panel title="Health" :icon="HeartPulse" to="/health">
        <ul v-if="d.health.length" class="divide-y divide-line border-t border-line">
          <li v-for="m in d.health.slice(0, 4)" :key="m.name" class="flex items-center gap-4 px-5 py-3">
            <div class="w-28 min-w-0">
              <p class="truncate text-[13px] text-ink2 first-letter:uppercase">{{ m.name }}</p>
              <p class="text-[17px] font-semibold tabular-nums">{{ m.latest !== null ? num(m.latest) : '–' }} <span class="text-[12.5px] font-normal text-ink3">{{ m.unit }}</span></p>
            </div>
            <div class="flex h-8 flex-1 items-end gap-[2px]" aria-hidden="true">
              <span v-for="(p, i) in m.points.slice(-24)" :key="p.day" class="flex-1 rounded-[2px]"
                :class="i === Math.min(23, m.points.length - 1) ? 'bg-ink' : 'bg-line2'"
                :style="{ height: (() => { const pts = m.points.slice(-24).map((x) => x.value); const lo = Math.min(...pts), hi = Math.max(...pts); return 20 + (hi === lo ? 40 : ((p.value - lo) / (hi - lo)) * 80) + '%' })() }" />
            </div>
            <span v-if="m.latest !== null && m.previous !== null && m.latest !== m.previous" class="w-12 text-right text-[12.5px] tabular-nums text-ink2">
              {{ m.latest > m.previous ? '+' : '−' }}{{ num(Math.abs(m.latest - m.previous)) }}
            </span>
          </li>
        </ul>
        <p v-else class="border-t border-line px-5 py-6 text-[14px] text-ink2">No measurements in the last 30 days.</p>
        <div class="flex items-center gap-2 border-t border-line px-5 py-3">
          <Timer class="size-4 text-ink3" aria-hidden="true" />
          <span class="flex-1 text-[13.5px] text-ink2">{{ d.journal_written_today ? 'Journal written today' : 'Journal not written yet today' }}</span>
          <AskChip :icon="PenLine" :words="d.journal_written_today ? 'Append to today’s journal entry: ' : 'Write today’s journal entry: '" label="Write" />
        </div>
      </Panel>
    </div>
  </template>
  <template v-for="plugin in plugins" :key="plugin.id">
    <PluginFrame v-for="slot in plugin.slots.filter((s) => s.slot === 'dashboard')" :key="slot.id" :plugin="plugin" :slot="slot" class="mt-4" />
  </template>
</template>

<style scoped>
.brief :deep(strong) { font-weight: 600; }
.brief :deep(ul) { margin-top: 0.6em; }
.brief :deep(li) { margin: 0.3em 0; }
</style>
