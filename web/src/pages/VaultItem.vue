<script setup lang="ts">
// One Vault item (agentes.chat ADR 0144 §7): its fields, who may use it beside them, and its recent uses. A new item
// is this same page, empty (`/connections/vault/new?kind=`). A saved password or key stays behind its dots: changing
// it means typing a new one, and the box empties the moment it is sent.
import { computed, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { BookUser, ChevronRight, Globe, KeyRound, Trash2 } from '@lucide/vue'
import { Button } from '../components/ui/button'
import IconTile from '../components/ds/IconTile.vue'
import StatusTag from '../components/ds/StatusTag.vue'
import LoadState from '../components/LoadState.vue'
import AgentAccess from '../components/vault/AgentAccess.vue'
import { useRead } from '../data'
import { ago } from '../format'
import { KIND_LABEL, KIND_TEXT, removeVaultItem, saveVaultItem, useVault } from '../vault'
import type { VaultKind } from '../core/generated/VaultKind'

const props = defineProps<{ id: string }>()
const route = useRoute()
const router = useRouter()
const { data, error, loading, retry } = useVault()
const { data: crew } = useRead('list_crew')
const names = computed(() => new Map((crew.value ?? []).map((m) => [m.id, m.name])))

const isNew = computed(() => props.id === 'new')
const item = computed(() => (isNew.value ? undefined : data.value?.items.find((i) => String(i.id) === props.id)))
const kind = computed<VaultKind>(() => {
  if (item.value) return item.value.kind
  const k = route.query.kind
  return k === 'key' || k === 'contact' ? k : 'login'
})
const uses = computed(() => (data.value?.uses ?? []).filter((u) => String(u.item) === props.id).slice(0, 12))

const form = ref({
  title: '', description: '', website: '', username: '', key_name: '',
  contact: { name: '', company: '', tax_id: '', email: '', phone: '', address: '' },
})
const secret = ref('')
const access = ref<{ all_agents: boolean; agents: string[] }>({ all_agents: false, agents: ['assistant'] })
const loaded = ref('')
// Fill the form once per item (and not again when the Vault is read again while the owner types).
watch(
  [item, isNew, () => route.query.name],
  () => {
    const key = isNew.value ? `new:${kind.value}` : String(item.value?.id ?? '')
    if (!key || key === loaded.value || (!isNew.value && !item.value)) return
    loaded.value = key
    const i = item.value
    form.value = {
      title: i?.title ?? (typeof route.query.name === 'string' ? route.query.name : ''),
      description: i?.description ?? '',
      website: i?.website ?? '',
      username: i?.username ?? '',
      key_name: i?.key_name ?? (typeof route.query.name === 'string' ? route.query.name : ''),
      contact: { name: '', company: '', tax_id: '', email: '', phone: '', address: '', ...(i?.contact ?? {}) },
    }
    access.value = i ? { all_agents: i.all_agents, agents: [...i.agents] } : { all_agents: false, agents: ['assistant'] }
    secret.value = ''
  },
  { immediate: true },
)

const sending = ref(false)
const saveError = ref('')
const savedNote = ref('')
async function save() {
  if (sending.value) return
  sending.value = true
  saveError.value = ''
  savedNote.value = ''
  const value = secret.value
  secret.value = ''
  try {
    const f = form.value
    const done = await saveVaultItem({
      id: item.value?.id,
      kind: kind.value,
      title: f.title,
      description: f.description,
      website: kind.value === 'login' ? f.website : undefined,
      username: kind.value === 'login' ? f.username : undefined,
      key_name: kind.value === 'key' ? f.key_name : undefined,
      contact: kind.value === 'contact' ? f.contact : undefined,
      secret: value || undefined,
      all_agents: access.value.all_agents,
      agents: access.value.agents,
    })
    savedNote.value = 'Saved.'
    if (isNew.value) {
      loaded.value = String(done.id)
      void router.replace(`/connections/vault/${done.id}`)
    }
    retry()
  } catch (err) {
    saveError.value = err instanceof Error ? err.message : String(err)
  } finally {
    sending.value = false
  }
}

async function remove() {
  const i = item.value
  if (!i) return
  const used = i.used_in.length ? ` ${i.used_in.join(', ')} stops until another key named ${i.key_name} is saved.` : ''
  if (!window.confirm(`Delete “${i.title}” from the Vault?${used}`)) return
  try {
    await removeVaultItem(i.id)
    void router.replace('/connections/vault')
  } catch (err) {
    saveError.value = err instanceof Error ? err.message : String(err)
  }
}

const ICON = { login: Globe, key: KeyRound, contact: BookUser } as const
const TONE = { login: 'info', key: 'warning', contact: 'people' } as const
const USE_WORDS: Record<string, string> = {
  key: 'Sent to', read: 'Read by', create: 'Saved', edit: 'Changed',
}
const field = 'h-10 w-full rounded-[10px] border border-line2 bg-panel px-3 text-[14px] text-ink placeholder:text-ink3 focus-visible:outline-2 focus-visible:outline-ring'
const label = 'flex flex-col gap-1.5 text-[13px] font-medium text-ink2'
const needsSecret = computed(() => kind.value !== 'contact' && (isNew.value || !item.value?.has_secret))
const canSave = computed(() => form.value.title.trim() && (!needsSecret.value || secret.value) && !sending.value)
</script>

<template>
  <nav aria-label="Breadcrumb" class="mb-6 flex items-center gap-1.5 text-[14px] text-ink2">
    <RouterLink to="/connections" class="hover:text-ink">Integrations</RouterLink>
    <ChevronRight class="size-3.5 text-ink3" aria-hidden="true" />
    <RouterLink to="/connections/vault" class="hover:text-ink">Vault</RouterLink>
    <ChevronRight class="size-3.5 text-ink3" aria-hidden="true" />
    <span class="truncate text-ink" aria-current="page">{{ isNew ? `New ${KIND_LABEL[kind].toLowerCase()}` : item?.title ?? '…' }}</span>
  </nav>

  <LoadState :loading="loading && !data" :error="error" label="Vault" class="mb-4" @retry="retry" />
  <p v-if="data && !isNew && !item" class="text-[14px] text-ink2">This item is no longer in the Vault. <RouterLink to="/connections/vault" class="font-medium text-ink underline">Back to the Vault</RouterLink></p>

  <div v-if="data && (isNew || item)" class="@container max-w-[1080px]">
    <div class="mb-6 flex items-center gap-3.5">
      <IconTile :icon="ICON[kind]" :tone="TONE[kind]" size="lg" />
      <div class="min-w-0">
        <h1 class="truncate text-[28px] leading-tight font-semibold tracking-[-0.03em]">{{ isNew ? `New ${KIND_LABEL[kind].toLowerCase()}` : item?.title }}</h1>
        <p class="text-[14px] text-ink2">{{ KIND_TEXT[kind] }}</p>
      </div>
    </div>

    <div class="grid items-start gap-8 @min-[52rem]:grid-cols-[minmax(0,1fr)_17rem]">
      <form class="min-w-0 space-y-5" autocomplete="off" @submit.prevent="save">
        <label :class="label">Title
          <input v-model="form.title" :class="field" maxlength="200" required
            :placeholder="kind === 'login' ? 'GitHub' : kind === 'key' ? 'GitHub token' : 'Home address'" />
        </label>

        <template v-if="kind === 'login'">
          <label :class="label">Website
            <input v-model="form.website" :class="field" inputmode="url" spellcheck="false" placeholder="github.com" required />
          </label>
          <label :class="label">Username
            <input v-model="form.username" :class="field" spellcheck="false" autocomplete="off" placeholder="you@example.com" />
          </label>
        </template>

        <label v-if="kind === 'key'" :class="label">Name
          <input v-model="form.key_name" :class="[field, 'font-mono']" spellcheck="false" maxlength="128" placeholder="GITHUB_TOKEN" required
            @input="form.key_name = form.key_name.toUpperCase().replace(/[^A-Z0-9_]/g, '_')" />
          <span class="text-[12.5px] font-normal text-ink3">Connections send it where their configuration says <code class="font-mono">${{ '{' }}{{ form.key_name || 'NAME' }}{{ '}' }}</code>.</span>
        </label>

        <!-- The one place a password or a key is typed: masked, sent once, emptied on send. -->
        <label v-if="kind !== 'contact'" :class="label">
          {{ kind === 'login' ? 'Password' : 'Value' }}
          <input v-model="secret" type="password" :class="[field, 'font-mono']" maxlength="65536" autocomplete="new-password" spellcheck="false"
            data-1p-ignore data-lpignore="true" data-bwignore data-protonpass-ignore
            :placeholder="item?.has_secret ? (item.key_hint ? `Saved, ending ${item.key_hint}: type a new one to replace it` : 'Saved: type a new one to replace it') : kind === 'login' ? 'The password' : 'The key'" />
          <span class="text-[12.5px] font-normal text-ink3">Kept outside your vault folder. It comes back to no one, you included; only LifeOS uses it, for the agents you pick.</span>
        </label>

        <fieldset v-if="kind === 'contact'" class="grid gap-4 @min-[36rem]:grid-cols-2">
          <legend class="sr-only">Contact details</legend>
          <label :class="label">Name<input v-model="form.contact.name" :class="field" /></label>
          <label :class="label">Company<input v-model="form.contact.company" :class="field" /></label>
          <label :class="label">Email<input v-model="form.contact.email" type="email" :class="field" /></label>
          <label :class="label">Phone<input v-model="form.contact.phone" type="tel" :class="field" /></label>
          <label :class="label">Tax ID<input v-model="form.contact.tax_id" :class="field" /></label>
          <label :class="[label, '@min-[36rem]:col-span-2']">Address
            <textarea v-model="form.contact.address" rows="2" :class="[field, 'h-auto py-2']" />
          </label>
          <p class="text-[12.5px] text-ink3 @min-[36rem]:col-span-2">Not a secret: the agents it is shared with read these details to fill in forms.</p>
        </fieldset>

        <label :class="label">Description
          <textarea v-model="form.description" rows="2" maxlength="2000" :class="[field, 'h-auto py-2']" placeholder="What it is for, so agents pick the right one" />
        </label>

        <div>
          <p class="mb-2 text-[13px] font-medium text-ink2">Who may use it</p>
          <AgentAccess v-model="access" />
        </div>

        <p v-if="saveError" role="alert" class="text-[13.5px] text-critical">{{ saveError }}</p>
        <div class="flex flex-wrap items-center gap-2">
          <Button type="submit" class="h-10 rounded-xl px-5" :disabled="!canSave">{{ isNew ? 'Save to the Vault' : 'Save changes' }}</Button>
          <span v-if="savedNote" class="text-[13.5px] text-good">{{ savedNote }}</span>
          <Button v-if="item" type="button" variant="ghost" class="ml-auto h-10 rounded-xl px-4 text-ink2 hover:text-critical" @click="remove">
            <Trash2 class="size-4" aria-hidden="true" /> Delete
          </Button>
        </div>
      </form>

      <aside v-if="item" class="space-y-4 @min-[52rem]:sticky @min-[52rem]:top-0">
        <div class="rounded-2xl border border-line bg-panel2 px-4 py-4 text-[13.5px]">
          <p class="mb-3 text-[12.5px] font-medium text-ink3">Details</p>
          <dl class="space-y-3">
            <div v-if="item.kind !== 'contact'">
              <dt class="text-ink3">{{ item.kind === 'login' ? 'Password' : 'Value' }}</dt>
              <dd class="mt-0.5"><StatusTag :tone="item.has_secret ? 'good' : 'warning'">{{ item.has_secret ? (item.key_hint ? `Saved · ••••${item.key_hint}` : 'Saved') : 'None yet' }}</StatusTag></dd>
            </div>
            <div v-if="item.used_in.length">
              <dt class="text-ink3">Sent by</dt>
              <dd class="mt-0.5 flex flex-wrap gap-1.5">
                <RouterLink v-for="c in item.used_in" :key="c" to="/connections" class="rounded-md bg-panel3 px-1.5 py-0.5 font-mono text-[12px] hover:text-ink">{{ c }}</RouterLink>
              </dd>
            </div>
            <div><dt class="text-ink3">Last used</dt><dd class="mt-0.5">{{ item.last_used ? ago(item.last_used) : 'Never' }}</dd></div>
            <div><dt class="text-ink3">Changed</dt><dd class="mt-0.5">{{ ago(item.updated) }}</dd></div>
          </dl>
        </div>
        <div class="rounded-2xl border border-line px-4 py-4">
          <p class="mb-2 text-[12.5px] font-medium text-ink3">Recent uses</p>
          <p v-if="!uses.length" class="text-[13.5px] text-ink2">None yet.</p>
          <ul v-else class="space-y-2 text-[13px]">
            <li v-for="(u, n) in uses" :key="n" class="flex items-baseline gap-2">
              <span class="min-w-0 flex-1 truncate">
                {{ USE_WORDS[u.action] ?? u.action }}
                <template v-if="u.action === 'key'"> {{ u.detail }} for {{ u.agent === 'assistant' ? 'the Assistant' : names.get(u.agent ?? '') ?? u.agent }}</template>
                <template v-else-if="u.agent"> {{ u.agent === 'assistant' ? 'the Assistant' : names.get(u.agent) ?? u.agent }}</template>
                <template v-else-if="u.detail"> · {{ u.detail }}</template>
              </span>
              <span class="shrink-0 text-[12px] text-ink3 tabular-nums">{{ ago(u.at) }}</span>
            </li>
          </ul>
        </div>
      </aside>
    </div>
  </div>
</template>
