<script setup lang="ts">
import { computed } from 'vue'
import { ChevronRight, Download, Monitor, MonitorOff, ShieldCheck, Trash2, TriangleAlert, Users } from '@lucide/vue'
import AskChip from '../components/AskChip.vue'
import PageHeader from '../components/ds/PageHeader.vue'
import Panel from '../components/ds/Panel.vue'
import StatusTag from '../components/ds/StatusTag.vue'
import IconTile from '../components/ds/IconTile.vue'
import LoadState from '../components/LoadState.vue'
import { useRead } from '../data'

const { data, error, loading, retry } = useRead('list_computers')
const { data: connections } = useRead('list_connections')
const { data: crew } = useRead('list_crew')
const names = computed(() => new Map((crew.value ?? []).map((m) => [m.id, m.name])))
const members = computed(() => connections.value?.connections.find((c) => c.name === 'desktop')?.members ?? [])

function missing(c: { accessibility: boolean; screen_recording: boolean }) {
  return [!c.accessibility && 'Accessibility', !c.screen_recording && 'Screen Recording'].filter(Boolean).join(' and ')
}
function when(at: string) {
  const d = new Date(at)
  return Number.isNaN(d.getTime()) ? at : d.toLocaleString()
}
</script>

<template>
  <nav aria-label="Breadcrumb" class="mb-4 flex items-center gap-1.5 text-[14px] text-ink2">
    <RouterLink to="/connections" class="hover:text-ink">Integrations</RouterLink>
    <ChevronRight class="size-3.5 text-ink3" aria-hidden="true" />
    <span class="text-ink" aria-current="page">Desktop</span>
  </nav>
  <PageHeader title="Desktop" sub="Let your agents see and work your Macs." />
  <div class="grid min-w-0 items-start gap-5 @min-[64rem]:grid-cols-[minmax(0,1.5fr)_minmax(0,1fr)]">
    <Panel title="Computers" :icon="Monitor" :count="data?.length">
      <LoadState :loading="loading && !data" :error="error" label="computers" class="px-5 pb-4" @retry="retry" />
      <p v-if="data && data.length === 0" class="border-t border-line px-5 py-6 text-[14px] text-ink2">
        No computer yet. Install LifeOS Computer on a Mac and it shows up here.
      </p>
      <ul v-else class="border-t border-line">
        <li v-for="c in data ?? []" :key="c.id" class="border-b border-line px-5 py-4 last:border-b-0">
          <div class="flex items-start gap-3.5">
            <IconTile :icon="c.online ? Monitor : MonitorOff" :tone="c.online ? (c.paused ? 'warning' : 'good') : 'neutral'" />
            <div class="min-w-0 flex-1">
              <p class="flex flex-wrap items-center gap-2 text-[15px] font-medium">
                {{ c.name }}
                <StatusTag v-if="c.online && !c.paused" tone="good" pulse>Online</StatusTag>
                <StatusTag v-else-if="c.online" tone="warning">Online · paused</StatusTag>
                <StatusTag v-else tone="neutral">Away</StatusTag>
              </p>
              <p class="text-[13px] text-ink2">
                {{ c.width }}×{{ c.height }} · LifeOS Computer {{ c.version }}<template v-if="!c.online"> · away since {{ when(c.seen) }}</template>
              </p>
              <p v-if="missing(c)" class="mt-2 flex items-start gap-1.5 rounded-lg bg-critical-wash px-2.5 py-1.5 text-[12.5px] text-critical">
                <TriangleAlert class="mt-0.5 size-3.5 shrink-0" aria-hidden="true" />
                macOS hasn't allowed {{ missing(c) }}: on that Mac, open the LifeOS Computer menu and choose it to grant.
              </p>
            </div>
            <div class="flex flex-wrap justify-end gap-2">
              <AskChip v-if="c.online && !c.paused" :icon="Monitor" label="Use it" :words="`On ${c.name}, `" />
              <AskChip ghost :icon="Trash2" label="Forget" :words="`Forget computer ${c.name}.`" />
            </div>
          </div>
        </li>
      </ul>
      <div class="flex flex-wrap items-center gap-3 border-t border-line px-5 py-3.5">
        <p class="flex min-w-0 flex-1 items-center gap-1.5 text-[13px] text-ink2">
          <Users class="size-3.5 text-ink3" aria-hidden="true" />
          Access · Assistant<template v-for="id in members" :key="id">, {{ names.get(id) ?? id }}</template>
        </p>
        <AskChip :icon="Users" label="Crew access" words="Change crew access to the desktop connection: " />
      </div>
    </Panel>

    <Panel title="Add a Mac" :icon="Download" pad>
      <ol class="space-y-3 text-[14px]">
        <li class="flex gap-3">
          <span class="grid size-6 shrink-0 place-items-center rounded-full bg-panel2 text-[12px] font-medium tabular-nums">1</span>
          <span>
            On the Mac, open this page and
            <a href="/downloads/LifeOS-Computer.zip" class="font-medium underline decoration-line2 underline-offset-4 hover:decoration-ink">download LifeOS Computer</a>.
            The Mac must be on your tailnet.
          </span>
        </li>
        <li class="flex gap-3">
          <span class="grid size-6 shrink-0 place-items-center rounded-full bg-panel2 text-[12px] font-medium tabular-nums">2</span>
          <span>Unzip it and move <b class="font-medium">LifeOS Computer</b> to Applications.</span>
        </li>
        <li class="flex gap-3">
          <span class="grid size-6 shrink-0 place-items-center rounded-full bg-panel2 text-[12px] font-medium tabular-nums">3</span>
          <span>
            Open it. The first time, macOS says it can't check the developer: open System Settings → Privacy &amp;
            Security and choose <b class="font-medium">Open Anyway</b>.
          </span>
        </li>
        <li class="flex gap-3">
          <span class="grid size-6 shrink-0 place-items-center rounded-full bg-panel2 text-[12px] font-medium tabular-nums">4</span>
          <span>
            From its icon in the menu bar, allow <b class="font-medium">Accessibility</b> and <b class="font-medium">Screen Recording</b>, and turn on
            <b class="font-medium">Open at login</b>. It connects to this server on its own; change it under <b class="font-medium">Server…</b>.
          </span>
        </li>
      </ol>
      <p class="mt-4 flex items-start gap-2 rounded-xl border border-line bg-panel2 px-3.5 py-2.5 text-[13px] text-ink2">
        <ShieldCheck class="mt-0.5 size-4 shrink-0 text-good" aria-hidden="true" />
        <span>Agents act only when asked. When you touch the mouse or keyboard they stop, and <b class="font-medium text-ink">Pause agents</b> in the
        menu bar turns them away until you resume.</span>
      </p>
    </Panel>
  </div>
</template>
