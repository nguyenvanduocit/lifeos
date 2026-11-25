<script setup lang="ts">
import { ArrowUpRight, Bot, ChevronDown, KeyRound, PanelsTopLeft, Radio, Workflow, Wrench } from '@lucide/vue'
import type { Plugin } from '../plugins'

defineProps<{ plugin: Plugin }>()
const effects: Record<string, string> = {
  read: 'Read only', reversible: 'Reversible change', compensable: 'Compensable change', irreversible: 'Irreversible action',
}
</script>

<template>
  <div class="@container space-y-5 border-t border-line bg-panel2/40 p-4 sm:p-5">
    <div v-if="plugin.agents?.length || plugin.automations?.length" class="grid gap-3 @min-[32rem]:grid-cols-2">
      <RouterLink v-for="agent in plugin.agents" :key="agent.id" :to="`/crew/plugin.${plugin.id}.${agent.id}`"
        class="group flex items-start gap-3 rounded-xl border border-line bg-panel p-3.5 hover:border-line2 focus-visible:outline-ring">
        <span data-tone="info" class="grid size-9 shrink-0 place-items-center rounded-lg bg-tone-wash text-tone-ink"><Bot class="size-5" /></span>
        <span class="min-w-0 flex-1">
          <span class="block text-xs text-ink2">Crew agent</span>
          <span class="block text-sm font-semibold group-hover:underline">{{ agent.name }}</span>
          <span v-if="agent.description" class="mt-1 block text-xs leading-relaxed text-ink2">{{ agent.description }}</span>
        </span>
        <ArrowUpRight class="size-4 shrink-0 text-ink3" aria-hidden="true" />
      </RouterLink>
      <RouterLink v-for="routine in plugin.automations" :key="routine.id"
        :to="{ path: '/automation', query: { tab: 'runs', routine: `plugin.${plugin.id}.${routine.id}` } }"
        class="group flex items-start gap-3 rounded-xl border border-line bg-panel p-3.5 hover:border-line2 focus-visible:outline-ring">
        <span data-tone="signal" class="grid size-9 shrink-0 place-items-center rounded-lg bg-tone-wash text-tone-ink"><Workflow class="size-5" /></span>
        <span class="min-w-0 flex-1">
          <span class="block text-xs text-ink2">Automation</span>
          <span class="block text-sm font-semibold group-hover:underline">{{ routine.title }}</span>
          <span class="mt-1 block text-xs text-ink2">{{ routine.paused ? 'Paused' : [routine.cron, ...(routine.on ?? [])].filter(Boolean).join(' · ') || 'Runs on demand' }}</span>
        </span>
        <ArrowUpRight class="size-4 shrink-0 text-ink3" aria-hidden="true" />
      </RouterLink>
    </div>

    <section v-if="plugin.functions.length">
      <h4 class="mb-2 flex items-center gap-2 text-sm font-semibold"><Wrench class="size-4 text-ink2" /> MCP tools</h4>
      <ul class="divide-y divide-line rounded-xl border border-line bg-panel px-4">
        <li v-for="tool in plugin.functions" :key="tool.name" class="py-3">
          <div class="flex flex-wrap items-center justify-between gap-2">
            <code class="break-all text-[13px] font-medium text-ink">{{ tool.name }}</code>
            <span v-if="tool.effect" class="rounded-md bg-panel2 px-2 py-0.5 text-xs text-ink2">{{ effects[tool.effect] ?? tool.effect }}</span>
          </div>
          <p class="mt-1 max-w-prose text-[13px] leading-relaxed text-ink2">{{ tool.description }}</p>
        </li>
      </ul>
    </section>

    <div v-if="plugin.processors?.length || Object.keys(plugin.emits ?? {}).length">
      <h4 class="mb-2 flex items-center gap-2 text-sm font-semibold"><Radio class="size-4 text-ink2" /> Events</h4>
      <div class="rounded-xl border border-line bg-panel px-4 py-3 text-[13px]">
        <p v-for="processor in plugin.processors" :key="processor" class="text-ink2">{{ processor }}</p>
        <details v-if="Object.keys(plugin.emits ?? {}).length" class="group" :class="{ 'mt-2': plugin.processors?.length }">
          <summary class="flex cursor-pointer list-none items-center gap-1 font-medium [&::-webkit-details-marker]:hidden">
            <ChevronDown class="size-3.5 -rotate-90 transition-transform group-open:rotate-0" /> Published events ({{ Object.keys(plugin.emits ?? {}).length }})
          </summary>
          <ul class="mt-3 space-y-2">
            <li v-for="(description, name) in plugin.emits" :key="name">
              <code class="break-all text-xs">{{ name }}</code><p class="text-ink2">{{ description }}</p>
            </li>
          </ul>
        </details>
      </div>
    </div>

    <section v-if="plugin.vault_keys?.length">
      <h4 class="mb-2 flex items-center gap-2 text-sm font-semibold"><KeyRound class="size-4 text-ink2" /> Vault keys</h4>
      <div class="flex flex-wrap gap-2">
        <RouterLink v-for="key in plugin.vault_keys" :key="key" :to="`/connections/plugins/${plugin.id}/settings`"
          class="inline-flex max-w-full items-center gap-1.5 rounded-lg border border-line bg-panel px-2.5 py-1.5 text-xs hover:border-line2 hover:underline">
          <code class="break-all">{{ key }}</code><ArrowUpRight class="size-3 shrink-0 text-ink3" />
        </RouterLink>
      </div>
    </section>

    <section v-if="plugin.slots.length">
      <h4 class="mb-2 flex items-center gap-2 text-sm font-semibold"><PanelsTopLeft class="size-4 text-ink2" /> Views</h4>
      <ul class="space-y-1 text-sm">
        <li v-for="slot in plugin.slots" :key="slot.slot + slot.id">
          <RouterLink v-if="slot.slot !== 'dashboard'" class="inline-flex items-center gap-1 hover:underline" :to="`/plugins/${plugin.id}/${slot.id}`">
            {{ slot.title || slot.id }} <ArrowUpRight class="size-3.5" />
          </RouterLink>
          <span v-else>{{ slot.title || slot.id }} · Dashboard</span>
        </li>
      </ul>
    </section>
  </div>
</template>
