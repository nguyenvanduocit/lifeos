<script setup lang="ts">
import { computed, ref } from 'vue'
import { ChevronLeft, ChevronRight } from '@lucide/vue'
import { useRead } from '../data'
import { compact, money, shortDay } from '../format'
import AskChip from '../components/AskChip.vue'
import Bars from '../components/Bars.vue'
import BudgetRow from '../components/BudgetRow.vue'
import Empty from '../components/Empty.vue'
import PageHead from '../components/PageHead.vue'
import Section from '../components/Section.vue'

const now = new Date()
const cursor = ref(new Date(now.getFullYear(), now.getMonth(), 1))
const month = computed(() => `${cursor.value.getFullYear()}-${String(cursor.value.getMonth() + 1).padStart(2, '0')}`)
const isThisMonth = computed(() => cursor.value.getFullYear() === now.getFullYear() && cursor.value.getMonth() === now.getMonth())
function shift(by: number) {
  cursor.value = new Date(cursor.value.getFullYear(), cursor.value.getMonth() + by, 1)
}

const { data: m } = useRead('money_month', () => ({ month: month.value }))
const primary = computed(() => m.value?.primary_currency ?? 'VND')
const currencies = computed(() =>
  [...(m.value?.currencies ?? [])].sort((a, b) => Number(b.currency === primary.value) - Number(a.currency === primary.value)),
)
const bars = computed(() => (m.value?.daily_spending ?? []).map((p) => ({ label: shortDay(p.day), value: p.amount })))
</script>

<template>
  <PageHead title="Tài chính" sub="Chi là số âm, thu là số dương; nói với agent mỗi khi tiền ra vào.">
    <AskChip words="Vừa chi " label="Ghi chi tiêu" />
    <AskChip words="Đặt ngân sách tháng cho " label="Đặt ngân sách" />
    <AskChip :words="`Nhận xét chi tiêu tháng ${month} của mình.`" label="Nhận xét" now />
  </PageHead>
  <div class="mb-4 flex items-center gap-2 text-sm">
    <button type="button" class="rounded p-1 hover:bg-panel2" aria-label="Tháng trước" @click="shift(-1)"><ChevronLeft :size="16" /></button>
    <span class="w-24 text-center font-medium">{{ month }}</span>
    <button type="button" class="rounded p-1 hover:bg-panel2 disabled:opacity-30" :disabled="isThisMonth" aria-label="Tháng sau" @click="shift(1)"><ChevronRight :size="16" /></button>
  </div>
  <div v-if="m" class="grid gap-4 md:grid-cols-3">
    <div v-for="c in currencies" :key="c.currency" class="rounded-xl border border-line bg-panel p-4 md:col-span-3">
      <div class="flex flex-wrap gap-8">
        <div><div class="text-xs text-ink3">Thu</div><div class="text-xl font-semibold tabular-nums">{{ money(c.income, c.currency) }}</div></div>
        <div><div class="text-xs text-ink3">Chi</div><div class="text-xl font-semibold tabular-nums">{{ money(c.spending, c.currency) }}</div></div>
        <div><div class="text-xs text-ink3">Còn lại</div><div class="text-xl font-semibold tabular-nums" :class="c.net < 0 ? 'text-critical' : ''">{{ money(c.net, c.currency) }}</div></div>
      </div>
    </div>
    <Section :title="`Chi mỗi ngày (${primary})`" class="md:col-span-2">
      <Bars v-if="bars.some((b) => b.value > 0)" :points="bars" :format="(v) => money(v, primary)" :height="160" />
      <Empty v-else text="Chưa có khoản chi nào trong tháng." words="Vừa chi " label="Ghi khoản chi" />
    </Section>
    <Section title="Ngân sách">
      <ul v-if="m.budgets.length"><BudgetRow v-for="b in m.budgets" :key="b.category + b.currency" :line="b" /></ul>
      <Empty v-else text="Chưa đặt ngân sách." words="Đặt ngân sách tháng cho ăn uống là " label="Đặt ngân sách" />
    </Section>
    <Section v-for="c in currencies" :key="'cat' + c.currency" :title="`Chi theo danh mục (${c.currency})`">
      <ul v-if="c.spending_by_category.length" class="space-y-1.5 text-sm">
        <li v-for="cat in c.spending_by_category" :key="cat.category" class="flex items-center gap-2">
          <span class="w-28 truncate first-letter:uppercase">{{ cat.category }}</span>
          <div class="h-2 flex-1 rounded bg-panel2">
            <div class="h-full rounded" :style="{ width: (cat.amount / c.spending_by_category[0].amount) * 100 + '%', background: 'var(--series)' }" />
          </div>
          <span class="w-20 text-right text-xs tabular-nums text-ink2">{{ compact(cat.amount) }}</span>
        </li>
      </ul>
      <Empty v-else text="Không có khoản chi." />
    </Section>
    <Section title="Giao dịch gần nhất" class="md:col-span-2">
      <table v-if="m.recent.length" class="w-full text-sm">
        <tbody>
          <tr v-for="t in m.recent" :key="t.id" class="border-b border-line last:border-0">
            <td class="py-1.5 text-xs text-ink3">{{ shortDay(t.day) }}</td>
            <td class="py-1.5 first-letter:uppercase">{{ t.category }}</td>
            <td class="py-1.5 text-ink2">{{ t.note }}</td>
            <td class="py-1.5 text-right tabular-nums" :class="t.amount > 0 ? 'text-good' : ''">{{ money(t.amount, t.currency) }}</td>
          </tr>
        </tbody>
      </table>
      <Empty v-else text="Không có giao dịch." />
    </Section>
  </div>
</template>
