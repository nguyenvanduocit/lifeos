<script setup lang="ts">
import { computed } from 'vue'
import {
  HeartPulse, Inbox, LayoutDashboard, ListChecks, MessageCircle, NotebookPen, Repeat, StickyNote, Target, Users, Wallet,
} from '@lucide/vue'
import AssistantPanel from './components/AssistantPanel.vue'
import { assistant, setOpen } from './assistant'
import { linkState } from './core/link'
import { PAGES } from './router'

const ICONS = { LayoutDashboard, ListChecks, Target, Repeat, Users, Wallet, HeartPulse, NotebookPen, StickyNote, Inbox }
const status = computed(() =>
  linkState.status === 'open' ? null : linkState.status === 'connecting' ? 'Đang kết nối…' : `Mất kết nối, đang thử lại… ${linkState.error}`,
)
</script>

<template>
  <div class="flex h-full">
    <nav class="flex w-14 shrink-0 flex-col items-center gap-1 border-r border-line bg-panel py-3 lg:w-52 lg:items-stretch lg:px-3">
      <RouterLink to="/" class="mb-4 px-2 text-base font-semibold tracking-tight">
        <span class="hidden lg:inline">lifeos</span><span class="lg:hidden">l</span>
      </RouterLink>
      <RouterLink
        v-for="p in PAGES"
        :key="p.path"
        :to="p.path"
        class="flex items-center gap-2.5 rounded-lg px-2 py-1.5 text-sm text-ink2 hover:bg-panel2 hover:text-ink"
        active-class="!bg-panel2 !text-ink font-medium"
        :exact-active-class="p.path === '/' ? '!bg-panel2 !text-ink font-medium' : ''"
        :title="p.name"
      >
        <component :is="ICONS[p.icon]" :size="17" />
        <span class="hidden lg:inline">{{ p.name }}</span>
      </RouterLink>
      <button
        v-if="!assistant.open"
        type="button"
        class="mt-auto flex items-center gap-2.5 rounded-lg bg-accent px-2 py-1.5 text-sm text-white"
        title="Mở Assistant"
        @click="setOpen(true)"
      >
        <MessageCircle :size="17" /> <span class="hidden lg:inline">Assistant</span>
      </button>
    </nav>

    <main class="min-w-0 flex-1 overflow-y-auto">
      <div v-if="status" class="border-b border-line bg-panel2 px-6 py-1.5 text-xs text-ink2">{{ status }}</div>
      <div class="mx-auto max-w-6xl px-4 py-6 lg:px-8">
        <RouterView />
      </div>
    </main>

    <div v-if="assistant.open" class="fixed inset-0 z-20 md:static md:inset-auto md:z-auto md:w-[380px] md:shrink-0">
      <AssistantPanel />
    </div>
  </div>
</template>
