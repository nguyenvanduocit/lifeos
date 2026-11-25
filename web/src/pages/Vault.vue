<script setup lang="ts">
// Settings › Vault (after agentes.chat's ADR 0144 §7): the owner's logins, keys and contacts, each with who may use it
// and when an agent last did. Values never show: a saved password or key stays behind its dots. The kind and the
// search live in the address.
import { computed, ref } from 'vue'
import { useRouter } from 'vue-router'
import { BookUser, Clock, FileUp, Globe, KeyRound, Plus, Search, Users, Vault as VaultIcon } from '@lucide/vue'
import IntegrationTabs from '../components/IntegrationTabs.vue'
import PageHeader from '../components/ds/PageHeader.vue'
import Segmented from '../components/ds/Segmented.vue'
import TableHead from '../components/ds/TableHead.vue'
import IconTile from '../components/ds/IconTile.vue'
import StatusTag from '../components/ds/StatusTag.vue'
import LoadState from '../components/LoadState.vue'
import EnvImport from '../components/vault/EnvImport.vue'
import { Button } from '../components/ui/button'
import { useRead } from '../data'
import { useQueryRef } from '../nav'
import { ago } from '../format'
import { KIND_LABEL, KIND_TEXT, itemLine, useVault, whoLine } from '../vault'
import type { VaultKind } from '../core/generated/VaultKind'

const router = useRouter()
const { data, error, loading, retry } = useVault()
const { data: crew } = useRead('list_crew')
const names = computed(() => new Map((crew.value ?? []).map((m) => [m.id, m.name])))

const VIEWS = ['all', 'login', 'key', 'contact'] as const
const view = useQueryRef('kind', 'all', VIEWS)
const query = useQueryRef<string>('q', '')
const items = computed(() => data.value?.items ?? [])
const count = (k: VaultKind) => items.value.filter((i) => i.kind === k).length
const options = computed(() => [
  { value: 'all' as const, label: 'All', count: items.value.length },
  { value: 'login' as const, label: 'Logins', count: count('login') },
  { value: 'key' as const, label: 'Keys', count: count('key') },
  { value: 'contact' as const, label: 'Contacts', count: count('contact') },
])
const shown = computed(() => {
  const words = query.value.toLowerCase().split(/\s+/).filter(Boolean)
  return items.value
    .filter((i) => view.value === 'all' || i.kind === view.value)
    .filter((i) => {
      const text = [i.title, i.description, itemLine(i)].join(' ').toLowerCase()
      return words.every((w) => text.includes(w))
    })
})

const ICON = { login: Globe, key: KeyRound, contact: BookUser } as const
const TONE = { login: 'info', key: 'warning', contact: 'people' } as const
const GRID = 'minmax(0,2.2fr) minmax(0,1.3fr) minmax(0,0.9fr)'
const cols = [
  { label: 'Item', icon: VaultIcon },
  { label: 'Who may use it', icon: Users, class: 'hidden @min-[44rem]:flex' },
  { label: 'Last used', icon: Clock },
]
const missingValue = (i: { kind: VaultKind; has_secret: boolean }) => i.kind !== 'contact' && !i.has_secret

const choosing = ref(false)
const importing = ref(false)
function start(kind: VaultKind) {
  choosing.value = false
  void router.push({ path: '/connections/vault/new', query: { kind } })
}
</script>

<template>
  <PageHeader title="Integrations" sub="What your agents can reach, and the keys they reach it with." />
  <IntegrationTabs />

  <section class="@container min-w-0 overflow-hidden rounded-2xl border border-line bg-panel">
    <div class="flex flex-wrap items-start justify-between gap-4 px-5 pt-5 pb-4">
      <div class="min-w-0">
        <h2 class="flex items-center gap-2 text-[20px] font-semibold tracking-[-0.02em]"><VaultIcon class="size-5" aria-hidden="true" />Vault</h2>
        <p class="mt-1 text-[14px] text-ink2">Your agents' passwords, keys and contacts. They never see a password or a key.</p>
      </div>
      <div class="flex flex-wrap gap-2">
        <Button variant="outline" size="sm" class="rounded-lg border-line2 bg-panel text-[13px] text-ink hover:bg-panel2" @click="importing = true">
          <FileUp class="size-3.5" aria-hidden="true" /> Import .env
        </Button>
        <Button size="sm" class="rounded-lg text-[13px]" :aria-expanded="choosing" @click="choosing = !choosing">
          <Plus class="size-3.5" aria-hidden="true" /> New item
        </Button>
      </div>
    </div>

    <!-- What a new item is: three kinds, each with one line. -->
    <div v-if="choosing" class="grid gap-2 border-t border-line px-5 py-4 @min-[40rem]:grid-cols-3">
      <button v-for="k in (['login', 'key', 'contact'] as const)" :key="k" type="button"
        class="flex items-start gap-3 rounded-xl border border-line2 bg-panel p-3.5 text-left transition-colors hover:border-ink3 hover:bg-panel2 focus-visible:outline-2 focus-visible:outline-ring"
        @click="start(k)">
        <IconTile :icon="ICON[k]" :tone="TONE[k]" size="sm" />
        <span class="min-w-0">
          <span class="block text-[14.5px] font-medium">{{ KIND_LABEL[k] }}</span>
          <span class="block text-[13px] text-ink2">{{ KIND_TEXT[k] }}</span>
        </span>
      </button>
    </div>

    <EnvImport v-if="importing" :saved="items.flatMap((i) => (i.key_name ? [i.key_name] : []))" class="border-t border-line" @close="importing = false" @done="importing = false; retry()" />

    <div class="flex flex-wrap items-center gap-3 border-t border-line px-5 py-3.5">
      <Segmented v-model="view" :options="options" label="Kind" />
      <label class="relative ml-auto w-full max-w-64 @max-[36rem]:max-w-none">
        <span class="sr-only">Search the Vault</span>
        <Search class="pointer-events-none absolute top-1/2 left-2.5 size-4 -translate-y-1/2 text-ink3" aria-hidden="true" />
        <input v-model="query" type="search" placeholder="Search the Vault" spellcheck="false"
          class="h-8 w-full rounded-[10px] border border-line2 bg-panel pr-2.5 pl-8 text-[13px] text-ink placeholder:text-ink3 focus-visible:outline-2 focus-visible:outline-ring" />
      </label>
    </div>

    <LoadState :loading="loading && !data" :error="error" label="Vault" class="px-5 pb-4" @retry="retry" />
    <div v-if="data && shown.length" role="table" aria-label="Vault">
      <TableHead :grid="GRID" grid-sm="minmax(0,1fr) auto" :cols="cols" />
      <div v-for="i in shown" :key="i.id" role="row" data-cols :style="{ '--cols': GRID, '--cols-sm': 'minmax(0,1fr) auto' }"
        class="relative grid items-center gap-4 border-b border-line px-5 py-3 last:border-b-0 hover:bg-panel2/60">
        <div role="cell" class="flex min-w-0 items-center gap-3">
          <IconTile :icon="ICON[i.kind]" :tone="TONE[i.kind]" size="sm" />
          <div class="min-w-0">
            <RouterLink :to="`/connections/vault/${i.id}`" class="flex items-center gap-2 truncate text-[14.5px] font-medium after:absolute after:inset-0 focus-visible:outline-2 focus-visible:outline-ring">
              <span class="truncate">{{ i.title }}</span>
              <StatusTag v-if="missingValue(i)" tone="warning">No value</StatusTag>
            </RouterLink>
            <p class="truncate text-[13px] text-ink2" :class="i.kind === 'key' ? 'font-mono text-[12.5px]' : ''">{{ itemLine(i) || KIND_LABEL[i.kind] }}</p>
          </div>
        </div>
        <div role="cell" class="hidden min-w-0 @min-[44rem]:block">
          <p class="truncate text-[13.5px]" :class="!i.all_agents && !i.agents.length ? 'text-ink3' : ''">{{ whoLine(i, names) }}</p>
          <p v-if="i.used_in.length" class="truncate text-[12.5px] text-ink3">Sent by {{ i.used_in.join(', ') }}</p>
        </div>
        <div role="cell" class="text-right text-[13px] text-ink2 tabular-nums @min-[44rem]:text-left">{{ i.last_used ? ago(i.last_used) : 'Never used' }}</div>
      </div>
    </div>
    <div v-else-if="data && items.length" class="border-t border-line px-5 py-10 text-center text-[14px] text-ink2">
      Nothing here matches.
      <button type="button" class="ml-1 font-medium text-ink underline-offset-2 hover:underline" @click="query = ''; view = 'all'">Clear the search</button>
    </div>
    <div v-else-if="data" class="flex flex-col items-center gap-3 border-t border-line px-5 py-14 text-center">
      <span class="grid size-12 place-items-center rounded-2xl bg-panel2"><VaultIcon class="size-6 text-ink2" aria-hidden="true" /></span>
      <p class="text-[16px] font-semibold">Share logins without passing passwords around</p>
      <p class="max-w-md text-[14px] text-ink2">Save a login, a key or contact details once, and choose which agents may use it.</p>
      <Button size="sm" class="mt-1 rounded-lg text-[13px]" @click="choosing = true"><Plus class="size-3.5" aria-hidden="true" /> New item</Button>
    </div>
  </section>
</template>
