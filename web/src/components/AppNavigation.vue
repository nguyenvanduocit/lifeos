<script setup lang="ts">
import {
  Brain, Bot, HeartPulse, Inbox, LayoutDashboard, ListChecks, NotebookPen, Repeat, StickyNote, Target, Users, Wallet,
} from '@lucide/vue'
import { PAGES } from '../router'

defineEmits<{ navigate: [] }>()

const icons = { LayoutDashboard, ListChecks, Target, Repeat, Users, Wallet, HeartPulse, NotebookPen, StickyNote, Inbox, Bot, Brain }
const groups = [...new Set(PAGES.map((page) => page.group))].map((title) => ({
  title,
  pages: PAGES.filter((page) => page.group === title),
}))
</script>

<template>
  <nav aria-label="Điều hướng chính" class="space-y-5 px-3 pb-5">
    <div v-for="group in groups" :key="group.title">
      <p class="mb-1.5 px-3 text-[10px] font-extrabold tracking-[.12em] text-ink2 uppercase">{{ group.title }}</p>
      <RouterLink
        v-for="page in group.pages"
        :key="page.path"
        :to="page.path"
        class="flex min-h-9 items-center gap-3 rounded-xl px-3 py-1.5 text-[13px] font-semibold text-ink2 hover:bg-panel2 hover:text-ink focus-visible:outline-2 focus-visible:outline-ring"
        active-class="bg-primary/12 !text-primary font-bold"
        @click="$emit('navigate')"
      >
        <component :is="icons[page.icon]" class="size-4 shrink-0" />
        {{ page.name }}
      </RouterLink>
    </div>
  </nav>
</template>
