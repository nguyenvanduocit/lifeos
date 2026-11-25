<script setup lang="ts">
// The pages as Relay lists them: a solid ink pill for the page that is open, quiet grey counts, and a lime count for
// what waits on the owner. Today first, then the life areas, then the agents; Settings sits at the foot (`settings`).
import { computed } from 'vue'
import { DOMAINS } from '../domains'
import { useRead } from '../data'
import { PAGES } from '../router'

const props = defineProps<{ settings?: boolean }>()
defineEmits<{ navigate: [] }>()

const groups = computed(() => {
  const titles = [...new Set(PAGES.map((page) => page.group))].filter((t) => (t === 'Settings') === !!props.settings)
  return titles.map((title) => ({ title, pages: PAGES.filter((page) => page.group === title) }))
})

const { data: d } = useRead('dashboard')
const counts = computed((): Record<string, { n: number; signal?: boolean; label: string } | undefined> => {
  if (!d.value) return {}
  const overdue = d.value.tasks.overdue.length
  const due = overdue + d.value.tasks.today.length
  const habitsLeft = d.value.habits.filter((h) => !h.done_today).length
  return {
    inbox: d.value.inbox.length ? { n: d.value.inbox.length, label: `${d.value.inbox.length} to triage` } : undefined,
    tasks: due ? { n: due, signal: overdue > 0, label: overdue ? `${overdue} overdue, ${due} for today` : `${due} for today` } : undefined,
    habits: habitsLeft ? { n: habitsLeft, label: `${habitsLeft} left today` } : undefined,
    people: d.value.people_due.length ? { n: d.value.people_due.length, label: `${d.value.people_due.length} to reach out to` } : undefined,
  }
})
</script>

<template>
  <nav :aria-label="settings ? 'Settings' : 'Main navigation'" class="space-y-6 px-3">
    <div v-for="group in groups" :key="group.title">
      <p v-if="group.title !== 'Today' && !settings" class="mb-1.5 px-3 text-[12px] font-medium tracking-[0.06em] text-ink3 uppercase">{{ group.title }}</p>
      <ul class="space-y-0.5">
        <li v-for="page in group.pages" :key="page.path">
          <RouterLink v-slot="{ isActive, href, navigate }" :to="page.path" custom>
            <a
              :href="href"
              :aria-current="isActive || (page.path !== '/' && $route.path.startsWith(page.path + '/')) ? 'page' : undefined"
              class="flex h-9 items-center gap-3 rounded-[10px] px-3 text-[14.5px] text-ink2 transition-colors hover:bg-panel3/70 hover:text-ink focus-visible:outline-2 focus-visible:outline-ring aria-[current=page]:bg-ink aria-[current=page]:font-medium aria-[current=page]:text-panel aria-[current=page]:shadow-sm"
              @click="navigate($event); $emit('navigate')"
            >
              <component :is="DOMAINS[page.domain].icon" class="size-[18px] shrink-0" stroke-width="1.75" aria-hidden="true" />
              <span class="min-w-0 flex-1 truncate">{{ page.name }}</span>
              <span
                v-if="counts[page.domain]"
                class="grid h-5 min-w-5 place-items-center rounded-md px-1.5 text-[12px] font-medium tabular-nums"
                :class="counts[page.domain]?.signal ? 'bg-signal text-signal-fg' : 'bg-panel3 text-ink2'"
                :title="counts[page.domain]?.label"
              >
                {{ counts[page.domain]?.n }}<span class="sr-only">: {{ counts[page.domain]?.label }}</span>
              </span>
            </a>
          </RouterLink>
        </li>
      </ul>
    </div>
  </nav>
</template>
