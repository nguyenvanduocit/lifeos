<script setup lang="ts">
// Integrations › Connections: what the agents can reach. Two groups and one row shape: what is connected (Google, the
// Macs, the MCP servers added), each with its state, who uses it, the Vault keys it sends and one action; then what
// can be added (the presets, and a custom server, which the host's terminal adds). Google and Desktop open their own
// pages from their rows; the Vault has its own tab.
import { computed, ref } from 'vue'
import { Check, ChevronRight, Copy, KeyRound, Plus, Power, Terminal, Trash2, Users } from '@lucide/vue'
import IntegrationTabs from '../components/IntegrationTabs.vue'
import AskChip from '../components/AskChip.vue'
import Markdown from '../components/Markdown.vue'
import PageHeader from '../components/ds/PageHeader.vue'
import StatusTag from '../components/ds/StatusTag.vue'
import LoadState from '../components/LoadState.vue'
import BrandMark from '../components/settings/BrandMark.vue'
import { Button } from '../components/ui/button'
import { useRead } from '../data'
import { useVault } from '../vault'
import type { Connection } from '../core/generated/Connection'

const { data, error, loading, retry } = useRead('list_connections')
const { data: crew } = useRead('list_crew')
const { data: vault } = useVault()
const presets = computed(() => new Map((data.value?.presets ?? []).map((p) => [p.id, p])))
const names = computed(() => new Map((crew.value ?? []).map((m) => [m.id, m.name])))
const keys = computed(() => new Map((vault.value?.items ?? []).filter((i) => i.key_name).map((i) => [i.key_name as string, i])))

// Who may use it, in words: the Assistant always, then the crew members given it.
const access = (members: string[]) => ['Assistant', ...members.map((id) => names.value.get(id) ?? id)].join(', ')

interface Row { id: string; mark: string; name: string; what: string; state: { label: string; tone: string }; to?: string; action: string; c: Connection }
const rows = computed((): Row[] =>
  (data.value?.connections ?? []).map((c) => {
    if (c.name === 'google') {
      return { id: c.name, mark: 'google', name: 'Google', what: c.enabled && c.target ? `Gmail, Calendar, Drive, Contacts · ${c.target}` : 'Gmail, Calendar, Drive, Contacts',
        state: c.enabled ? { label: 'Signed in', tone: 'good' } : { label: 'Not signed in', tone: 'neutral' }, to: '/connections/google', action: c.enabled ? 'Manage' : 'Sign in', c }
    }
    if (c.name === 'desktop') {
      return { id: c.name, mark: 'desktop', name: 'Desktop', what: c.enabled ? `Your Macs, through LifeOS Computer · ${c.target}` : 'Your Macs, through LifeOS Computer',
        state: c.enabled ? { label: 'On', tone: 'good' } : { label: 'No Mac yet', tone: 'neutral' }, to: '/connections/desktop', action: c.enabled ? 'Manage' : 'Add a Mac', c }
    }
    const missing = c.missing.length > 0
    return { id: c.name, mark: c.preset ?? 'custom', name: presets.value.get(c.preset ?? '')?.name ?? c.name, what: `${c.transport.toUpperCase()} · ${c.target}`,
      state: !c.enabled ? { label: 'Off', tone: 'neutral' } : missing ? { label: 'Needs a key', tone: 'warning' } : { label: 'On', tone: 'good' }, action: '', c }
  }),
)
const available = computed(() => (data.value?.presets ?? []).filter((p) => !p.added))

const howOpen = ref(false)
const copied = ref(false)
const EXAMPLE = `echo '{"url": "https://mcp.example.com/mcp", "headers": {"Authorization": "Bearer \${EXAMPLE_TOKEN}"}}' \\
  | lifeos connection set example`
async function copy() {
  await navigator.clipboard.writeText(EXAMPLE)
  copied.value = true
  setTimeout(() => { copied.value = false }, 1500)
}
</script>

<template>
  <PageHeader title="Integrations" sub="What your agents can reach, and the keys they reach it with." />
  <IntegrationTabs />
  <LoadState :loading="loading && !data" :error="error" label="connections" class="mb-4" @retry="retry" />

  <div v-if="data" class="@container max-w-[960px] space-y-8">
    <section aria-labelledby="connected">
      <div class="mb-3 flex items-baseline gap-2">
        <h2 id="connected" class="text-[17px] font-semibold tracking-[-0.01em]">Your connections</h2>
        <span class="text-[13px] text-ink3 tabular-nums">{{ rows.length }}</span>
        <p class="ml-auto hidden text-[13px] text-ink2 sm:block">What your agents can reach. Keys come from the <RouterLink to="/connections/vault" class="font-medium text-ink hover:underline">Vault</RouterLink>.</p>
      </div>
      <ul class="overflow-hidden rounded-2xl border border-line bg-panel">
        <li v-for="r in rows" :key="r.id" class="relative border-b border-line last:border-b-0" :class="r.to ? 'hover:bg-panel2/60' : ''">
          <div class="flex items-center gap-4 px-5 py-4">
            <BrandMark :name="r.mark" />
            <div class="min-w-0 flex-1">
              <p class="flex flex-wrap items-center gap-2">
                <RouterLink v-if="r.to" :to="r.to" class="text-[15px] font-medium after:absolute after:inset-0 focus-visible:outline-2 focus-visible:outline-ring">{{ r.name }}</RouterLink>
                <span v-else class="text-[15px] font-medium">{{ r.name }}</span>
                <StatusTag :tone="r.state.tone">{{ r.state.label }}</StatusTag>
              </p>
              <p class="truncate text-[13px] text-ink2" :title="r.what">{{ r.what }}</p>
              <p class="mt-1 flex items-center gap-1.5 truncate text-[12.5px] text-ink3"><Users class="size-3.5 shrink-0" aria-hidden="true" />{{ access(r.c.members) }}</p>
            </div>
            <span v-if="r.to" class="hidden items-center gap-1 text-[13.5px] font-medium text-ink2 sm:flex">{{ r.action }}<ChevronRight class="size-4 text-ink3" aria-hidden="true" /></span>
            <AskChip v-else :icon="Power" :label="r.c.enabled ? 'Turn off' : 'Turn on'" :words="`${r.c.enabled ? 'Disable' : 'Enable'} connection ${r.c.name}.`" />
          </div>

          <!-- An MCP server: the Vault keys it sends, then what else the Assistant can change. -->
          <div v-if="!r.to" class="-mt-1 space-y-2 px-5 pb-4 sm:pl-[4.75rem]">
            <div v-for="s in r.c.secrets" :key="s" class="flex flex-wrap items-center gap-x-2 gap-y-1 text-[13px]">
              <KeyRound class="size-3.5 text-ink3" aria-hidden="true" />
              <code class="font-mono text-[12.5px]">{{ s }}</code>
              <template v-if="keys.get(s)">
                <span class="text-ink3">·</span>
                <RouterLink :to="`/connections/vault/${keys.get(s)!.id}`" class="text-ink2 hover:text-ink hover:underline">In the Vault</RouterLink>
              </template>
              <RouterLink v-else :to="{ path: '/connections/vault/new', query: { kind: 'key', name: s } }"
                class="inline-flex items-center gap-1 rounded-md bg-tone-wash px-1.5 py-0.5 font-medium text-tone-ink hover:underline" data-tone="warning">
                <Plus class="size-3.5" aria-hidden="true" /> Save it in the Vault
              </RouterLink>
            </div>
            <div class="flex flex-wrap gap-1.5 pt-1">
              <AskChip ghost :icon="Users" label="Who uses it" :words="`Change crew access to the ${r.c.name} connection: `" />
              <AskChip ghost :icon="Trash2" label="Remove" :words="`Remove connection ${r.c.name}.`" />
            </div>
          </div>
        </li>
      </ul>
    </section>

    <section aria-labelledby="add">
      <h2 id="add" class="mb-3 text-[17px] font-semibold tracking-[-0.01em]">Add more</h2>
      <div class="grid gap-3 @min-[40rem]:grid-cols-2 @min-[56rem]:grid-cols-3">
        <article v-for="p in available" :key="p.id" class="flex flex-col rounded-2xl border border-line bg-panel p-4">
          <div class="flex items-center gap-3">
            <BrandMark :name="p.id" />
            <p class="text-[15px] font-medium">{{ p.name }}</p>
          </div>
          <p class="mt-3 flex-1 text-[13.5px] text-ink2">{{ p.description }}</p>
          <p v-if="p.secrets.length" class="mt-2 flex items-center gap-1.5 text-[12.5px] text-ink3"><KeyRound class="size-3.5" aria-hidden="true" />Uses <code class="font-mono">{{ p.secrets.join(', ') }}</code> from the Vault</p>
          <details class="group mt-3 text-[13px]">
            <summary class="cursor-pointer list-none text-ink2 hover:text-ink [&::-webkit-details-marker]:hidden">
              <ChevronRight class="mr-0.5 inline size-3.5 transition-transform group-open:rotate-90" aria-hidden="true" />Before you add it
            </summary>
            <Markdown class="mt-2 text-[13px] wrap-anywhere text-ink2" :source="p.setup" />
          </details>
          <AskChip class="mt-3 self-start" :icon="Plus" :label="`Add ${p.name}`" :words="`Add a connection from the ${p.id} preset.`" />
        </article>

        <article class="flex flex-col rounded-2xl border border-dashed border-line2 p-4">
          <div class="flex items-center gap-3">
            <BrandMark name="custom" />
            <p class="text-[15px] font-medium">Custom MCP server</p>
          </div>
          <p class="mt-3 flex-1 text-[13.5px] text-ink2">Any MCP server over HTTP or stdio. It starts a process or sends your keys elsewhere, so it is added from the LifeOS host's terminal.</p>
          <Button variant="outline" class="mt-3 h-8 self-start rounded-[10px] border-line2 bg-panel px-3 text-[13px] text-ink hover:bg-panel2" :aria-expanded="howOpen" @click="howOpen = !howOpen">
            <Terminal class="size-3.5" aria-hidden="true" /> {{ howOpen ? 'Hide' : 'How' }}
          </Button>
        </article>
      </div>

      <div v-if="howOpen" class="mt-3 rounded-2xl border border-line bg-panel2 p-4 text-[13.5px] text-ink2">
        <p>On the LifeOS host, pipe the server's configuration into <code class="font-mono text-[12.5px] text-ink">lifeos connection set &lt;name&gt;</code>. HTTP takes <code class="font-mono text-[12.5px] text-ink">url</code> and <code class="font-mono text-[12.5px] text-ink">headers</code>; stdio takes <code class="font-mono text-[12.5px] text-ink">command</code>, <code class="font-mono text-[12.5px] text-ink">args</code> and <code class="font-mono text-[12.5px] text-ink">env</code>.</p>
        <div class="relative mt-3">
          <pre class="overflow-x-auto rounded-xl border border-line bg-panel p-3 pr-12 font-mono text-[12.5px] text-ink">{{ EXAMPLE }}</pre>
          <button type="button" class="absolute top-2 right-2 grid size-8 place-items-center rounded-lg text-ink2 hover:bg-panel2 hover:text-ink" :aria-label="copied ? 'Copied' : 'Copy the command'" @click="copy">
            <Check v-if="copied" class="size-4 text-good" aria-hidden="true" /><Copy v-else class="size-4" aria-hidden="true" />
          </button>
        </div>
        <p class="mt-3"><code class="font-mono text-[12.5px] text-ink">${EXAMPLE_TOKEN}</code> is a key you save in the <RouterLink to="/connections/vault" class="font-medium text-ink hover:underline">Vault</RouterLink>: the server reaches only the agents that key is shared with.</p>
      </div>
    </section>
  </div>
</template>
