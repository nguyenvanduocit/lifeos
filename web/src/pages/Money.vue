<script setup lang="ts">
import EntityId from '../components/ds/EntityId.vue'
// One month of money (docs/design.md, "Dashboard" over "List"): what went out against the budgets, day by day, where
// it went, and every transaction of the month: none hidden; the table shows the newest and says how many more there
// are. The month, a category and "show all" live in the address (?month=, ?cat=, ?all=1).
import { computed } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import {
  ArrowDownLeft, CalendarClock, CalendarDays, ChartColumn, ChevronLeft, ChevronRight, PiggyBank, Plus, Receipt, Scale, ShoppingBasket, Tag,
  TrendingDown, Wallet, X,
} from '@lucide/vue'
import { useRead } from '../data'
import { useQueryRef } from '../nav'
import { isoDay, money, moneyShort, relativeDay, shortDay, todayIso } from '../format'
import LoadState from '../components/LoadState.vue'
import AskChip from '../components/AskChip.vue'
import Bars from '../components/Bars.vue'
import PageHeader from '../components/ds/PageHeader.vue'
import StatStrip from '../components/ds/StatStrip.vue'
import Panel from '../components/ds/Panel.vue'
import TableHead from '../components/ds/TableHead.vue'
import IconTile from '../components/ds/IconTile.vue'
import { categoryIcon } from '../components/life/icons'

const route = useRoute()
const router = useRouter()
const thisMonth = todayIso().slice(0, 7)
const monthQ = useQueryRef<string>('month', thisMonth)
const month = computed(() => (/^\d{4}-\d{2}$/.test(monthQ.value) && monthQ.value <= thisMonth ? monthQ.value : thisMonth))
const category = useQueryRef<string>('cat', '')
const showAll = computed(() => route.query.all === '1')
const toggleAll = () => void router.replace({ query: { ...route.query, all: showAll.value ? undefined : '1' } })

const first = computed(() => new Date(Number(month.value.slice(0, 4)), Number(month.value.slice(5)) - 1, 1))
const last = computed(() => new Date(first.value.getFullYear(), first.value.getMonth() + 1, 0))
const monthLabel = computed(() => new Intl.DateTimeFormat('en-US', { month: 'long', year: 'numeric' }).format(first.value))
const isThisMonth = computed(() => month.value === thisMonth)
const daysLeft = computed(() => (isThisMonth.value ? last.value.getDate() - new Date().getDate() : 0))
function shift(by: number) {
  const d = new Date(first.value.getFullYear(), first.value.getMonth() + by, 1)
  monthQ.value = isoDay(d).slice(0, 7)
}

const { data: m, error, loading, retry } = useRead('money_month', () => ({ month: month.value }))
const { data: all, loading: txLoading, error: txError, retry: txRetry } = useRead('list_transactions', () => ({ from: isoDay(first.value), to: isoDay(last.value) }))

const primary = computed(() => m.value?.primary_currency ?? 'VND')
const main = computed(() => m.value?.currencies.find((c) => c.currency === primary.value) ?? m.value?.currencies[0])
const others = computed(() => (m.value?.currencies ?? []).filter((c) => c !== main.value))
const budgets = computed(() => [...(m.value?.budgets ?? [])].sort((a, b) => b.spent / b.budget - a.spent / a.budget))
const budgetSpent = computed(() => budgets.value.filter((b) => b.currency === primary.value).reduce((s, b) => s + b.spent, 0))
const budgetTotal = computed(() => budgets.value.filter((b) => b.currency === primary.value).reduce((s, b) => s + b.budget, 0))
const bars = computed(() => (m.value?.daily_spending ?? []).map((p) => ({ label: shortDay(p.day), value: p.amount })))
const average = computed(() => {
  const total = bars.value.reduce((s, b) => s + b.value, 0)
  return bars.value.length && total ? total / bars.value.length : 0
})

const strip = computed(() => {
  const c = main.value
  if (!c) return []
  const pct = budgetTotal.value ? Math.round((budgetSpent.value / budgetTotal.value) * 100) : null
  const saved = c.income > 0 ? Math.round((c.net / c.income) * 100) : null
  return [
    { key: 'spent', icon: TrendingDown, value: moneyShort(c.spending, c.currency), label: 'spent', delta: pct !== null ? `${pct}% of budgets` : undefined, deltaTone: (pct !== null && pct > 100 ? 'bad' : 'flat') as 'bad' | 'flat' },
    { key: 'income', icon: ArrowDownLeft, value: moneyShort(c.income, c.currency), label: 'income' },
    { key: 'net', icon: Scale, value: (c.net < 0 ? '−' : '') + moneyShort(Math.abs(c.net), c.currency), label: 'net', urgent: c.net < 0,
      delta: saved !== null && saved > 0 ? `${saved}% saved` : undefined, deltaTone: 'good' as const },
    ...(isThisMonth.value ? [{ key: 'left', icon: CalendarClock, value: String(daysLeft.value), label: daysLeft.value === 1 ? 'day left' : 'days left' }] : []),
  ]
})

// Where the money went, largest first; choosing one filters the transactions.
const breakdown = computed(() => {
  const cats = main.value?.spending_by_category ?? []
  const total = cats.reduce((s, c) => s + c.amount, 0)
  return total ? cats.map((c) => ({ ...c, share: c.amount / total })) : []
})
const maxCat = computed(() => Math.max(0, ...breakdown.value.map((c) => c.amount)))

const LIMIT = 15
const txs = computed(() => [...(all.value ?? [])].reverse().filter((t) => !category.value || t.category.toLowerCase() === category.value.toLowerCase()))
const shown = computed(() => (showAll.value ? txs.value : txs.value.slice(0, LIMIT)))
const GRID = 'minmax(0,0.8fr) minmax(0,2.2fr) minmax(0,1fr)'
const cols = [
  { label: 'Day', icon: CalendarDays, class: 'hidden @min-[44rem]:flex' },
  { label: 'Category', icon: Tag },
  { label: 'Amount', icon: Wallet, class: 'justify-end' },
]
const day = (d: string) => (d === todayIso() ? 'Today' : relativeDay(d).replace(/^./, (c) => c.toUpperCase()))
const pct = (spent: number, budget: number) => Math.round((spent / budget) * 100)
const empty = computed(() => !!m.value && !m.value.recent.length && !m.value.budgets.length && !m.value.currencies.length)
</script>

<template>
  <PageHeader title="Money" :eyebrow="`${monthLabel}${isThisMonth ? ` · ${daysLeft} days left` : ''}`">
    <div class="flex h-8 items-center rounded-lg border border-line2 bg-panel" role="group" aria-label="Month">
      <button type="button" class="grid h-full w-8 place-items-center rounded-l-lg text-ink2 hover:bg-panel2 hover:text-ink" aria-label="Previous month" @click="shift(-1)">
        <ChevronLeft class="size-3.5" aria-hidden="true" />
      </button>
      <span class="min-w-26 px-1 text-center text-[13px] font-medium" aria-live="polite">{{ monthLabel }}</span>
      <button type="button" class="grid h-full w-8 place-items-center rounded-r-lg text-ink2 hover:bg-panel2 hover:text-ink disabled:opacity-30 disabled:hover:bg-transparent"
        :disabled="isThisMonth" aria-label="Next month" @click="shift(1)">
        <ChevronRight class="size-3.5" aria-hidden="true" />
      </button>
    </div>
    <RouterLink v-if="!isThisMonth" :to="{ query: { ...route.query, month: undefined } }" class="inline-flex h-8 items-center gap-1.5 rounded-lg px-2.5 text-[13px] font-medium text-ink2 hover:bg-panel2 hover:text-ink"><CalendarDays class="size-3.5" aria-hidden="true" />This month</RouterLink>
    <AskChip size="sm" :icon="PiggyBank" words="Set a monthly budget for " label="Set budget" />
    <AskChip size="sm" primary :icon="Plus" words="I just spent " label="Log expense" />
    <template #strip><StatStrip v-if="strip.length" :items="strip" /></template>
  </PageHeader>

  <LoadState :loading="loading && !m" :error="error" label="money" class="mb-4" @retry="retry" />

  <div v-if="empty" class="flex flex-col items-center gap-3 rounded-2xl border border-line bg-panel px-5 py-16 text-center">
    <span class="grid size-11 place-items-center rounded-xl bg-panel2"><Wallet class="size-5 text-ink2" /></span>
    <p class="text-[15px] font-medium">Nothing logged in {{ monthLabel }}</p>
    <p class="max-w-sm text-[14px] text-ink2">Tell the Assistant what you spend or earn, in your own words; budgets and categories fill in from there.</p>
    <AskChip :icon="Plus" words="I just spent " label="Log expense" />
  </div>

  <div v-else-if="m" class="@container grid gap-5">
    <div v-if="others.length" class="flex flex-wrap gap-2">
      <div v-for="c in others" :key="c.currency" class="flex flex-wrap items-center gap-x-4 gap-y-1 rounded-xl border border-line bg-panel px-3.5 py-2 text-[13.5px]">
        <span class="rounded-md bg-panel3 px-1.5 text-[12px] font-semibold text-ink2">{{ c.currency }}</span>
        <span class="text-ink2">in <b class="font-medium text-ink tabular-nums">{{ money(c.income, c.currency) }}</b></span>
        <span class="text-ink2">out <b class="font-medium text-ink tabular-nums">{{ money(c.spending, c.currency) }}</b></span>
        <span class="text-ink2">net <b class="font-medium tabular-nums" :class="c.net < 0 ? 'text-critical' : 'text-ink'">{{ money(c.net, c.currency) }}</b></span>
      </div>
    </div>

    <div class="grid items-start gap-5 @min-[60rem]:grid-cols-[minmax(0,1.6fr)_minmax(0,1fr)]">
      <Panel :icon="ChartColumn" title="Daily spending" :count="average ? `avg ${moneyShort(average, primary)} / day` : undefined" pad>
        <div style="--tone: var(--ink)">
          <Bars v-if="bars.some((b) => b.value > 0)" :points="bars" :format="(v) => money(v, primary)" :height="200" />
          <p v-else class="py-10 text-center text-[14px] text-ink3">No spending in {{ monthLabel }} yet.</p>
        </div>
      </Panel>

      <Panel :icon="PiggyBank" title="Budgets" :count="budgetTotal ? `${moneyShort(budgetSpent, primary)} / ${moneyShort(budgetTotal, primary)}` : undefined">
        <template #actions><AskChip :icon="Plus" words="Set a monthly budget for " label="Add" /></template>
        <ul v-if="budgets.length" class="border-t border-line">
          <li v-for="b in budgets" :key="b.category + b.currency" class="border-b border-line px-5 py-3 last:border-b-0">
            <div class="flex items-center gap-3">
              <IconTile :icon="categoryIcon(b.category)" size="sm" :tone="b.spent > b.budget ? 'critical' : 'neutral'" />
              <button type="button" class="min-w-0 flex-1 truncate text-left text-[14px] font-medium first-letter:uppercase hover:underline" @click="category = b.category">{{ b.category }}</button>
              <span class="text-[13px] text-ink2 tabular-nums">{{ moneyShort(b.spent, b.currency) }} <span class="text-ink3">/ {{ moneyShort(b.budget, b.currency) }}</span></span>
            </div>
            <div class="mt-2 flex items-center gap-3 pl-10">
              <span class="h-1.5 flex-1 overflow-hidden rounded-full bg-panel3">
                <span class="block h-full rounded-full" :class="b.spent > b.budget ? 'bg-critical' : pct(b.spent, b.budget) >= 85 ? 'bg-warning' : 'bg-ink'"
                  :style="{ width: Math.min(100, pct(b.spent, b.budget)) + '%' }" />
              </span>
              <span class="w-11 text-right text-[12.5px] tabular-nums" :class="b.spent > b.budget ? 'font-medium text-critical' : 'text-ink3'">{{ pct(b.spent, b.budget) }}%</span>
            </div>
          </li>
        </ul>
        <div v-else class="border-t border-line px-5 py-8 text-center text-[14px] text-ink3">
          No budgets yet. A monthly cap per category warns you before it breaks.
        </div>
      </Panel>
    </div>

    <div class="grid items-start gap-5 @min-[60rem]:grid-cols-[minmax(0,1.6fr)_minmax(0,1fr)]">
      <Panel :icon="Receipt" title="Transactions" :count="all ? (category ? `${txs.length} of ${all.length}` : all.length) : undefined">
        <template #title-extra>
          <button v-if="category" type="button" class="inline-flex h-6 items-center gap-1 rounded-md bg-ink px-2 text-[12.5px] font-medium text-panel first-letter:uppercase" @click="category = ''">
            {{ category }} <X class="size-3" aria-label="Clear category" />
          </button>
        </template>
        <template #actions><AskChip :icon="Plus" words="I just spent " label="Log" /></template>
        <LoadState :loading="txLoading && !all" :error="txError" label="transactions" class="px-5 pb-4" @retry="txRetry" />
        <div v-if="all" role="table" aria-label="Transactions">
          <TableHead :grid="GRID" :cols="cols" />
          <div v-for="t in shown" :key="t.id" role="row" data-cols class="grid items-center gap-x-4 border-b border-line px-5 py-2.5 last:border-b-0 hover:bg-panel2/60"
            :style="{ '--cols': GRID, '--cols-sm': 'minmax(0,1fr) auto' }">
            <time role="cell" :datetime="t.day" class="hidden truncate text-[13px] text-ink2 @min-[44rem]:block" :title="t.day">{{ day(t.day) }}</time>
            <div role="cell" class="flex min-w-0 items-center gap-3">
              <IconTile :icon="categoryIcon(t.category)" size="sm" :tone="t.amount > 0 ? 'good' : 'neutral'" />
              <div class="min-w-0">
                <p class="truncate text-[14px] font-medium first-letter:uppercase">{{ t.category }}</p>
                <p class="truncate text-[12.5px] text-ink3"><span class="@min-[44rem]:hidden">{{ day(t.day) }}<template v-if="t.note"> · </template></span>{{ t.note }}</p>
                <EntityId :value="`transaction_${t.id}`" />
              </div>
            </div>
            <span role="cell" class="text-right text-[14px] font-medium whitespace-nowrap tabular-nums" :class="t.amount > 0 ? 'text-good' : 'text-ink'">
              {{ t.amount > 0 ? '+' : '−' }}{{ money(Math.abs(t.amount), t.currency) }}
            </span>
          </div>
          <p v-if="!txs.length" class="px-5 py-10 text-center text-[14px] text-ink3">No transactions{{ category ? ` in ${category}` : '' }} in {{ monthLabel }}.</p>
          <div v-if="txs.length > LIMIT" class="flex items-center justify-between gap-3 border-t border-line bg-panel2 px-5 py-2.5 text-[13px] text-ink2">
            <span>Showing {{ shown.length }} of {{ txs.length }}</span>
            <button type="button" class="font-medium text-ink hover:underline" @click="toggleAll">{{ showAll ? 'Show newest only' : `Show all ${txs.length}` }}</button>
          </div>
        </div>
      </Panel>

      <Panel :icon="ShoppingBasket" title="Where it went" :count="breakdown.length ? `${breakdown.length} categories` : undefined">
        <ul v-if="breakdown.length" class="border-t border-line py-1.5">
          <li v-for="c in breakdown" :key="c.category">
            <button type="button" class="flex w-full items-center gap-3 px-5 py-2 text-left hover:bg-panel2/60 aria-pressed:bg-panel2"
              :aria-pressed="category.toLowerCase() === c.category.toLowerCase()" @click="category = category.toLowerCase() === c.category.toLowerCase() ? '' : c.category">
              <IconTile :icon="categoryIcon(c.category)" size="sm" tone="neutral" />
              <span class="min-w-0 flex-1">
                <span class="flex items-baseline gap-2">
                  <span class="min-w-0 flex-1 truncate text-[14px] font-medium first-letter:uppercase">{{ c.category }}</span>
                  <span class="text-[13px] tabular-nums">{{ moneyShort(c.amount, main?.currency ?? primary) }}</span>
                  <span class="w-9 text-right text-[12.5px] text-ink3 tabular-nums">{{ Math.round(c.share * 100) }}%</span>
                </span>
                <span class="mt-1.5 block h-1 overflow-hidden rounded-full bg-panel3">
                  <span class="block h-full rounded-full bg-ink/80" :style="{ width: (c.amount / maxCat) * 100 + '%' }" />
                </span>
              </span>
            </button>
          </li>
        </ul>
        <p v-else class="border-t border-line px-5 py-8 text-center text-[14px] text-ink3">No spending to split yet.</p>
      </Panel>
    </div>
  </div>
</template>
