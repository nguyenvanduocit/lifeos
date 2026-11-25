<script setup lang="ts">
// People as one table (docs/design.md, "List"): who is due for a word, whose birthday is near, when we last spoke and
// how often we mean to. The view lives in the address (?view=); each row opens the person's page.
import { computed } from 'vue'
import { Cake, CalendarClock, Clock, HeartHandshake, MessageCircle, Plus, Repeat, UserRound, Users } from '@lucide/vue'
import { useRead } from '../data'
import { useQueryRef } from '../nav'
import LoadState from '../components/LoadState.vue'
import AskChip from '../components/AskChip.vue'
import Avatar from '../components/Avatar.vue'
import PageHeader from '../components/ds/PageHeader.vue'
import StatStrip from '../components/ds/StatStrip.vue'
import Segmented from '../components/ds/Segmented.vue'
import TableHead from '../components/ds/TableHead.vue'
import StatusTag from '../components/ds/StatusTag.vue'
import { birthdaySoon, birthdayText, cadence, lastTouch } from '../components/people/touch'
import type { PersonView } from '../core/generated/PersonView'

const VIEWS = ['all', 'due', 'birthdays'] as const
const view = useQueryRef('view', 'all', VIEWS)
const { data, error, loading, retry } = useRead('list_people', () => ({ overdue_only: false }))

const people = computed(() => data.value ?? [])
const due = computed(() => people.value.filter((p) => p.overdue))
const soon = computed(() => people.value.filter(birthdaySoon).sort((a, b) => (a.days_to_birthday ?? 0) - (b.days_to_birthday ?? 0)))
const rank = (p: PersonView) => (p.overdue ? 0 : birthdaySoon(p) ? 1 : 2)
const rows = computed(() => {
  if (view.value === 'due') return due.value
  if (view.value === 'birthdays') return soon.value
  return [...people.value].sort((a, b) => rank(a) - rank(b) || a.name.localeCompare(b.name))
})
const viewOptions = computed(() => [
  { value: 'all' as const, label: 'All', count: people.value.length },
  { value: 'due' as const, label: 'Due', count: due.value.length, urgent: due.value.length > 0 },
  { value: 'birthdays' as const, label: 'Birthdays soon', count: soon.value.length },
])
const strip = computed(() => {
  if (!data.value) return []
  const week = people.value.filter((p) => p.days_since_contact !== null && p.days_since_contact <= 7).length
  return [
    { key: 'people', icon: Users, value: String(people.value.length), label: 'people' },
    { key: 'due', icon: HeartHandshake, value: String(due.value.length), label: 'due for a word', urgent: due.value.length > 0 },
    { key: 'bday', icon: Cake, value: String(soon.value.length), label: 'birthdays in 30 days' },
    { key: 'week', icon: MessageCircle, value: String(week), label: 'spoken to this week' },
  ]
})

const cap = (s: string | null) => (s ? s[0].toUpperCase() + s.slice(1) : '')
const GRID = 'minmax(0,2.2fr) minmax(0,1fr) minmax(0,0.9fr) minmax(0,0.9fr) minmax(0,1.1fr)'
const cols = [
  { label: 'Person', icon: UserRound },
  { label: 'Relation', icon: Users, class: 'hidden @min-[44rem]:flex' },
  { label: 'Last touch', icon: Clock, class: 'hidden @min-[44rem]:flex' },
  { label: 'Keep in touch', icon: Repeat, class: 'hidden @min-[44rem]:flex' },
  { label: 'Status', icon: CalendarClock },
]
const empty = computed(() => ({
  all: 'No people yet.',
  due: 'Nobody is waiting on a word from you.',
  birthdays: 'No birthdays in the next 30 days.',
}[view.value]))
</script>

<template>
  <PageHeader title="People" :eyebrow="data ? `${people.length} people · ${due.length} due` : undefined">
    <AskChip size="sm" :icon="MessageCircle" words="I just spoke with " label="Log a contact" />
    <AskChip size="sm" primary :icon="Plus" words="Add a person: " label="Add person" />
    <template #strip><StatStrip v-if="strip.length" :items="strip" /></template>
  </PageHeader>

  <section class="@container min-w-0 overflow-hidden rounded-2xl border border-line bg-panel">
    <div class="flex flex-wrap items-center gap-3 px-5 py-3.5">
      <Segmented v-model="view" :options="viewOptions" label="View" />
    </div>
    <LoadState :loading="loading && !data" :error="error" label="people" class="px-5 pb-4" @retry="retry" />
    <div v-if="data" role="table" aria-label="People">
      <TableHead :grid="GRID" :cols="cols" />
      <div v-for="p in rows" :key="p.id" role="row" data-cols
        class="relative grid items-center gap-x-4 border-b border-line px-5 py-3 last:border-b-0 hover:bg-panel2/60"
        :style="{ '--cols': GRID, '--cols-sm': 'minmax(0,1fr) auto' }">
        <div role="cell" class="flex min-w-0 items-center gap-3">
          <Avatar :name="p.name" :size="32" />
          <div class="min-w-0">
            <RouterLink :to="`/people/${encodeURIComponent(p.id)}`" class="block truncate text-[14px] font-medium after:absolute after:inset-0 focus-visible:outline-none">{{ p.name }}</RouterLink>
            <p class="truncate text-[12.5px] text-ink3 @min-[44rem]:hidden">
              {{ [cap(p.relation), lastTouch(p) === 'Never' ? 'Never in touch' : lastTouch(p), cadence(p)].filter(Boolean).join(' · ') }}
            </p>
          </div>
        </div>
        <div role="cell" class="hidden truncate text-[13.5px] text-ink2 first-letter:uppercase @min-[44rem]:block">{{ p.relation ?? '—' }}</div>
        <div role="cell" class="hidden text-[13.5px] tabular-nums @min-[44rem]:block" :class="p.overdue ? 'text-critical' : 'text-ink2'">{{ lastTouch(p) }}</div>
        <div role="cell" class="hidden text-[13.5px] text-ink2 @min-[44rem]:block">{{ cadence(p) || '—' }}</div>
        <div role="cell" class="flex flex-wrap items-center justify-end gap-1.5 @min-[44rem]:justify-start">
          <StatusTag v-if="birthdaySoon(p)" tone="warning" :dot="false"><Cake class="-ml-0.5 size-3" aria-hidden="true" /><span class="hidden @min-[44rem]:inline">{{ birthdayText(p) }}</span><span class="@min-[44rem]:hidden">{{ p.days_to_birthday }}d</span></StatusTag>
          <StatusTag v-if="p.overdue" tone="critical">Due</StatusTag>
          <span v-if="!p.overdue && !birthdaySoon(p)" class="text-[13px] text-ink3">In touch</span>
        </div>
      </div>
      <div v-if="!rows.length" class="flex flex-col items-center gap-3 px-5 py-14 text-center">
        <span class="grid size-11 place-items-center rounded-xl bg-panel2"><Users class="size-5 text-ink2" /></span>
        <p class="text-[15px] font-medium">{{ empty }}</p>
        <AskChip v-if="view === 'all'" :icon="Plus" words="Add a person: " label="Add person" />
      </div>
    </div>
  </section>
</template>
