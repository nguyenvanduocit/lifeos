<script setup lang="ts">
import EntityId from '../components/ds/EntityId.vue'
import PageActions from '../components/ds/PageActions.vue'
// A person on their own page, laid out like a task's: the trail back to People, their name with how things stand,
// the times we spoke as a timeline beside notes, and their facts in a Details card at the right.
import { computed } from 'vue'
import { useRoute } from 'vue-router'
import { BellRing, Cake, ChevronRight, Clock, MessageCircle, Pencil, Phone, Repeat, Sparkles, Tag, Users } from '@lucide/vue'
import { useRead } from '../data'
import { longDay, relativeDay, shortDay } from '../format'
import LoadState from '../components/LoadState.vue'
import Markdown from '../components/Markdown.vue'
import AskChip from '../components/AskChip.vue'
import Avatar from '../components/Avatar.vue'
import PropertyList from '../components/ds/PropertyList.vue'
import StatusTag from '../components/ds/StatusTag.vue'
import { channelIcon } from '../components/life/icons'
import { birthdaySoon, birthdayText, cadence, lastTouch, lateBy } from '../components/people/touch'

const props = defineProps<{ id: string }>()
const route = useRoute()
const fromSearch = computed(() => (typeof route.query.q === 'string' ? route.query.q : ''))
const { data, error, loading, retry } = useRead('person', () => ({ id: props.id }))
const p = computed(() => data.value?.person)
const ref_ = computed(() => (p.value ? `person_${p.value.id}` : ''))

function birthdayLabel(b: string) {
  const [month, day] = b.split('-').slice(-2)
  return shortDay(`2000-${month}-${day}`)
}
const rows = computed(() => {
  const x = p.value
  if (!x) return []
  return [
    { key: 'status', label: 'Status', icon: Clock, value: 'yes' },
    { key: 'relation', label: 'Relation', icon: Users, value: x.relation },
    { key: 'cadence', label: 'Keep in touch', icon: Repeat, value: cadence(x) },
    { key: 'last', label: 'Last touch', icon: MessageCircle, value: lastTouch(x) },
    { key: 'birthday', label: 'Birthday', icon: Cake, value: x.birthday },
    { key: 'tags', label: 'Tags', icon: Tag, value: x.tags.length ? 'yes' : null },
  ]
})
const late = computed(() => (p.value ? lateBy(p.value) : null))
const since = (day: string) => {
  const r = relativeDay(day)
  return r.charAt(0).toUpperCase() + r.slice(1)
}
</script>

<template>
  <nav aria-label="Breadcrumb" class="mb-6 flex items-center gap-1.5 text-[14px] text-ink2">
    <RouterLink v-if="fromSearch" :to="{ path: '/notes', query: { q: fromSearch } }" class="hover:text-ink">Search results</RouterLink>
    <RouterLink v-else to="/people" class="hover:text-ink">People</RouterLink>
    <ChevronRight class="size-3.5 text-ink3" aria-hidden="true" />
    <span class="truncate text-ink" aria-current="page">{{ p?.name ?? '…' }}</span>
  </nav>

  <LoadState :loading="loading && !data" :error="error" label="person" class="mb-4" @retry="retry" />
  <div v-if="data && p" class="@container max-w-[1080px]">
    <header class="flex flex-wrap items-center gap-4">
      <Avatar :name="p.name" :size="56" />
      <div class="min-w-0 flex-1">
        <h1 class="text-[30px] leading-tight font-semibold tracking-[-0.02em] wrap-anywhere">{{ p.name }}</h1>
        <EntityId :key="ref_" :value="ref_" />
        <p class="mt-1 flex flex-wrap items-center gap-x-2 text-[14px] text-ink2">
          <span v-if="p.relation" class="first-letter:uppercase">{{ p.relation }}</span>
          <span v-if="p.relation" class="text-ink3">·</span>
          <span>{{ p.last_contact ? `Last spoke ${lastTouch(p).toLowerCase()}` : 'Never in touch' }}</span>
        </p>
      </div>
      <PageActions>
        <AskChip size="sm" :icon="BellRing" :words="`Remind me to reach out to ${ref_} on `" label="Remind me" />
        <AskChip size="sm" primary :icon="Phone" :words="`I just spoke with ${ref_}: `" label="Log a contact" />
      </PageActions>
    </header>

    <div class="mt-8 grid items-start gap-8 @min-[52rem]:grid-cols-[minmax(0,1fr)_17rem]">
      <aside class="order-2 rounded-2xl border border-line bg-panel2 px-4 py-4 @min-[52rem]:sticky @min-[52rem]:top-0" aria-label="Details">
        <p class="mb-2 text-[12.5px] font-medium text-ink3">Details</p>
        <PropertyList :rows="rows" stack>
          <template #status>
            <StatusTag v-if="p.overdue" tone="critical">Due{{ late && late > 0 ? ` · ${late}d late` : '' }}</StatusTag>
            <StatusTag v-else-if="p.contact_every_days" tone="good">In touch</StatusTag>
            <StatusTag v-else tone="neutral">No cadence</StatusTag>
          </template>
          <template #relation><span class="first-letter:uppercase">{{ p.relation }}</span></template>
          <template #last>
            <span :class="p.overdue ? 'text-critical' : ''">{{ lastTouch(p) }}</span>
            <span v-if="p.last_contact" class="text-ink3">· {{ shortDay(p.last_contact) }}</span>
          </template>
          <template #birthday>
            <span>{{ p.birthday ? birthdayLabel(p.birthday) : '' }}</span>
            <StatusTag v-if="birthdaySoon(p)" tone="warning" :dot="false"><Cake class="-ml-0.5 size-3" aria-hidden="true" />{{ birthdayText(p) }}</StatusTag>
            <span v-else-if="p.days_to_birthday !== null" class="text-ink3">· in {{ p.days_to_birthday }}d</span>
          </template>
          <template #tags>
            <span v-for="g in p.tags" :key="g" class="rounded-md bg-panel3 px-1.5 py-0.5 text-[12.5px] text-ink2">#{{ g }}</span>
          </template>
        </PropertyList>
      </aside>

      <div class="order-1 min-w-0">
        <section>
          <h2 class="mb-3 flex items-center gap-2 text-[13px] font-medium text-ink3">Times we spoke <span class="tabular-nums">{{ data.interactions.length }}</span></h2>
          <ol v-if="data.interactions.length" class="relative">
            <li v-for="(i, n) in data.interactions" :key="i.id" class="relative flex gap-3.5 pb-5 last:pb-0">
              <span v-if="n < data.interactions.length - 1" class="absolute top-9 bottom-0 left-[17px] w-px bg-line" aria-hidden="true" />
              <span class="relative grid size-9 shrink-0 place-items-center rounded-xl border border-line bg-panel2 text-ink2">
                <component :is="channelIcon(i.channel)" class="size-4" aria-hidden="true" />
              </span>
              <div class="min-w-0 flex-1 pt-1">
                <p class="flex flex-wrap items-baseline gap-x-2 text-[13px]">
                  <time :datetime="i.day" class="font-medium text-ink" :title="longDay(i.day)">{{ since(i.day) }}</time>
                  <span v-if="i.channel" class="text-ink3 first-letter:uppercase">{{ i.channel }}</span>
                </p>
                <p v-if="i.note" class="mt-0.5 text-[14px] leading-relaxed text-ink2">{{ i.note }}</p>
                <EntityId :value="`interaction_${i.id}`" />
              </div>
            </li>
          </ol>
          <div v-else class="rounded-xl border border-dashed border-line2 px-4 py-8 text-center text-[14px] text-ink3">
            No times logged yet.
            <AskChip class="ml-2" ghost :icon="Phone" :words="`I just spoke with ${ref_}: `" label="Log a contact" />
          </div>
        </section>

        <section class="mt-8">
          <h2 class="mb-2 text-[13px] font-medium text-ink3">Notes</h2>
          <Markdown v-if="data.body.trim()" :source="data.body" class="text-[14.5px] leading-relaxed" />
          <div v-else class="rounded-xl border border-dashed border-line2 px-4 py-8 text-center text-[14px] text-ink3">
            No notes yet.
            <AskChip class="ml-2" ghost :icon="Pencil" :words="`Remember this about ${ref_}: `" label="Add a note" />
          </div>
        </section>

        <section class="mt-8">
          <h2 class="mb-2.5 text-[13px] font-medium text-ink3">Ask the Assistant</h2>
          <div class="flex flex-wrap gap-2">
            <AskChip :icon="Sparkles" :words="`Suggest how I can check in with ${ref_}.`" label="Ideas to check in" />
            <AskChip :icon="Repeat" :words="`Keep in touch with ${ref_} every `" label="Set cadence" />
            <AskChip :icon="Pencil" :words="`Update ${ref_}: `" label="Update details" />
            <AskChip :icon="MessageCircle" :words="`About ${ref_}: `" label="Ask about them" />
          </div>
        </section>
      </div>
    </div>
  </div>
</template>
