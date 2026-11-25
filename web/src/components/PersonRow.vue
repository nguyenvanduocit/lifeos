<script setup lang="ts">
import { computed } from 'vue'
import { AlertCircle, Cake } from '@lucide/vue'
import type { PersonView } from '../core/generated/PersonView'

const props = defineProps<{ person: PersonView; show?: 'contact' | 'birthday' }>()
const contact = computed(() => {
  const p = props.person
  if (p.days_since_contact === null) return p.contact_every_days ? 'chưa liên lạc' : ''
  return p.days_since_contact === 0 ? 'hôm nay' : `${p.days_since_contact} ngày trước`
})
const initials = computed(() =>
  props.person.name.split(/\s+/).filter(Boolean).slice(-2).map((w) => w[0]).join('').toUpperCase(),
)
</script>

<template>
  <li class="flex items-center gap-3 py-1.5">
    <span class="flex size-8 shrink-0 items-center justify-center rounded-full bg-panel2 text-xs font-medium text-ink2">{{ initials }}</span>
    <div class="min-w-0 flex-1">
      <RouterLink :to="`/people/${person.id}`" class="block truncate text-sm font-medium hover:underline">{{ person.name }}</RouterLink>
      <div class="truncate text-xs text-ink3">{{ person.relation }}</div>
    </div>
    <span v-if="show === 'birthday'" class="inline-flex shrink-0 items-center gap-1 text-xs text-ink2">
      <Cake :size="13" /> {{ person.days_to_birthday === 0 ? 'hôm nay' : `${person.days_to_birthday} ngày nữa` }}
    </span>
    <span
      v-else
      class="inline-flex shrink-0 items-center gap-1 text-xs"
      :class="person.overdue ? 'text-critical' : 'text-ink3'"
      :title="person.overdue ? `Nên liên lạc: mục tiêu mỗi ${person.contact_every_days} ngày` : undefined"
    >
      <AlertCircle v-if="person.overdue" :size="13" aria-label="Nên liên lạc" />{{ contact }}
    </span>
  </li>
</template>
