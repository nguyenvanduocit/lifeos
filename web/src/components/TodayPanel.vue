<script setup lang="ts">
import { onMounted, shallowRef } from 'vue'
import { today, type Today } from '../core/link'

const day = shallowRef<Today | null>(null)
const error = shallowRef('')

async function refresh() {
  try {
    day.value = await today()
    error.value = ''
  } catch (err) {
    error.value = String(err)
  }
}

const money = (n: number) => n.toLocaleString('vi-VN')

onMounted(refresh)
defineExpose({ refresh })
</script>

<template>
  <section class="card">
    <h1>{{ day ? `${day.weekday}, ${day.date}` : 'Hôm nay' }}</h1>
    <p v-if="error" class="error">{{ error }}</p>
    <p v-else-if="!day" class="muted">Đang tải…</p>
    <template v-else>
      <template v-if="day.overdue_tasks.length">
        <h2>Quá hạn</h2>
        <ul>
          <li v-for="t in day.overdue_tasks" :key="t.id" class="overdue">
            {{ t.title }} <span class="muted">({{ t.due }})</span>
          </li>
        </ul>
      </template>
      <template v-if="day.due_today.length">
        <h2>Hạn hôm nay</h2>
        <ul><li v-for="t in day.due_today" :key="t.id">{{ t.title }}</li></ul>
      </template>
      <p v-if="day.open_tasks_without_due" class="muted">{{ day.open_tasks_without_due }} việc chưa có hạn.</p>

      <template v-if="day.active_goals.length">
        <h2>Mục tiêu đang theo</h2>
        <ul>
          <li v-for="g in day.active_goals" :key="g.id">
            {{ g.title }}<span v-if="g.horizon" class="muted"> · {{ g.horizon }}</span>
          </li>
        </ul>
      </template>

      <template v-if="day.habits.length">
        <h2>Thói quen (7 ngày)</h2>
        <ul>
          <li v-for="h in day.habits" :key="h.habit">
            {{ h.done_today ? '✓' : '○' }} {{ h.habit }}
            <span class="muted">· chuỗi {{ h.streak }} · {{ h.days_done_in_window }}/{{ h.window_days }}</span>
          </li>
        </ul>
      </template>

      <template v-if="day.money_today.length">
        <h2>Tiền hôm nay</h2>
        <ul>
          <li v-for="m in day.money_today" :key="m.currency">
            chi {{ money(m.spending) }} {{ m.currency }}<span v-if="m.income"> · thu {{ money(m.income) }}</span>
          </li>
        </ul>
      </template>

      <h2>Nhật ký</h2>
      <p class="muted">{{ day.journal_written ? 'Đã viết hôm nay.' : 'Chưa viết hôm nay.' }}</p>
    </template>
  </section>
</template>
