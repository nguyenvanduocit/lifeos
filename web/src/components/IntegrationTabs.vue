<script setup lang="ts">
// The Integrations pages as one set of tabs (Relay's segmented well): each tab is its own address. Google and Desktop
// are rows in Connections and open from there, with a breadcrumb back.
import { Bot, Plug, Puzzle, Vault } from '@lucide/vue'

const TABS = [
  { to: '/connections', label: 'Connections', icon: Plug },
  { to: '/connections/vault', label: 'Vault', icon: Vault },
  { to: '/connections/accounts', label: 'Claude accounts', icon: Bot },
  { to: '/connections/plugins', label: 'Plugins', icon: Puzzle },
]
</script>

<template>
  <nav aria-label="Integrations" class="mb-6 inline-flex h-9 max-w-full items-center gap-0.5 overflow-x-auto rounded-[10px] bg-panel2 p-0.5 [scrollbar-width:none]">
    <RouterLink v-for="t in TABS" :key="t.to" :to="t.to"
      class="inline-flex h-8 shrink-0 items-center gap-2 rounded-lg px-3 whitespace-nowrap text-[13.5px] font-medium text-ink2 transition-colors hover:text-ink focus-visible:outline-2 focus-visible:outline-ring"
      :class="t.to === '/connections/vault' && $route.path.startsWith('/connections/vault') ? '!bg-panel !text-ink shadow-[0_0_0_1px_var(--line2)]' : ''"
      exact-active-class="!bg-panel !text-ink shadow-[0_0_0_1px_var(--line2)]">
      <component :is="t.icon" class="size-4" aria-hidden="true" />{{ t.label }}
    </RouterLink>
  </nav>
</template>
