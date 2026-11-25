<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue'
import { Check, KeyRound, LogIn, RefreshCw, Trash2, UserRound, X } from '@lucide/vue'
import IntegrationTabs from '../components/IntegrationTabs.vue'
import PageHeader from '../components/ds/PageHeader.vue'
import Panel from '../components/ds/Panel.vue'
import StatusTag from '../components/ds/StatusTag.vue'
import LoadState from '../components/LoadState.vue'
import { Button } from '../components/ui/button'
import type { AgentAccount } from '../core/generated/AgentAccount'
import type { AgentAccounts } from '../core/generated/AgentAccounts'
import { revision } from '../core/link'

const data = ref<AgentAccounts>()
const selected = ref('')
const account = computed(() => data.value?.accounts.find(a => a.id === selected.value) ?? data.value?.accounts[0])
const signin = computed(() => data.value?.signin)
const signingIn = computed(() => signin.value?.status === 'waiting')
const pendingAction = ref<string>()
const pending = computed(() => pendingAction.value !== undefined)
const error = ref('')
const loadError = ref('')
const code = ref('')
const token = ref('')
const label = ref('')
const replaces = ref<string>()
const tokenForm = ref(false)
const tokenNameInput = ref<HTMLInputElement>()
let tokenTrigger: HTMLElement | null = null
const signinSection = ref<HTMLElement>()
async function beginSignin(replace?: string) {
  if (await act({ action: 'login', replace })) {
    await nextTick()
    signinSection.value?.focus()
  }
}

const statuses: Record<string, string> = {
  ready: 'Ready', near: 'Near limit', limited: 'Rate limited',
  access: 'Authentication required', unchecked: 'Unchecked', checking: 'Checking',
}
// Older account checks stored their display text in the database.
const legacyReasons: Record<string, string> = {
  'Claude đã chạm giới hạn sử dụng.': 'Claude reached its usage limit.',
  'Claude từ chối xác thực. Đăng nhập lại hoặc kiểm tra thông tin kết nối.': 'Claude rejected authentication. Sign in again or check your credentials.',
  'Chưa xác nhận được Claude có thể nhận việc.': 'Could not confirm that Claude is ready.',
}
const TONES: Record<string, string> = { ready: 'good', near: 'warning', limited: 'critical', access: 'critical', unchecked: 'neutral', checking: 'info' }
const tone = (status: string) => TONES[status] ?? 'neutral'
const kinds: Record<string, string> = { default: 'Host account', login: 'Claude sign-in', connection: 'Claude connection' }
const quotas: Record<string, string> = {
  five_hour: '5 hours', seven_day: '7 days', seven_day_sonnet: '7 days · Sonnet', seven_day_opus: '7 days · Opus',
}

let request = 0
let timer: ReturnType<typeof setInterval> | undefined
async function readAccounts(payload?: object) {
  const number = ++request
  const response = await fetch('/api/accounts', {
    credentials: 'same-origin',
    cache: 'no-store',
    headers: { 'X-LifeOS-Account': '1', ...(payload ? { 'Content-Type': 'application/json' } : {}) },
    ...(payload ? { method: 'POST', body: JSON.stringify(payload) } : {}),
  })
  if (!response.headers.get('content-type')?.includes('application/json')) {
    throw new Error('Could not reach the account service.')
  }
  const result = await response.json()
  if (!response.ok) throw new Error(result.error ?? 'Account request failed.')
  if (number === request) data.value = result
}

async function refresh() {
  if (pending.value) return
  try {
    await readAccounts()
    loadError.value = ''
  } catch (err) {
    loadError.value = err instanceof Error ? err.message : String(err)
  }
}

async function act(payload: { action: string; [key: string]: unknown }) {
  if (pending.value) return false
  pendingAction.value = payload.action
  error.value = ''
  try {
    await readAccounts(payload)
    return true
  } catch (err) {
    error.value = err instanceof Error ? err.message : String(err)
    return false
  } finally {
    pendingAction.value = undefined
  }
}

function submitCode() {
  const value = code.value.trim()
  code.value = ''
  if (value && signin.value) void act({ action: 'code', id: signin.value.id, code: value })
}

async function editToken(previous?: AgentAccount) {
  tokenTrigger = document.activeElement instanceof HTMLElement ? document.activeElement : null
  error.value = ''
  token.value = ''
  label.value = previous?.label ?? ''
  replaces.value = previous?.id
  tokenForm.value = true
  await nextTick()
  tokenNameInput.value?.focus()
}

async function closeTokenForm() {
  token.value = ''
  tokenForm.value = false
  await nextTick()
  if (tokenTrigger?.isConnected) tokenTrigger.focus({ preventScroll: true })
}

async function submitToken() {
  const value = token.value.trim()
  token.value = ''
  if (value && await act({ action: 'token', label: label.value, token: value, replace: replaces.value })) {
    await closeTokenForm()
  }
}

function remove(account: AgentAccount) {
  if (window.confirm(`Remove ${account.label} from LifeOS?`)) void act({ action: 'remove', id: account.id })
}

function accountLabel(item: AgentAccount): string {
  return item.kind === 'default' ? 'Host account' : item.label
}

function resetTime(epoch: number) {
  return new Intl.DateTimeFormat('en-US', { dateStyle: 'medium', timeStyle: 'short' }).format(epoch * 1000)
}

watch(revision, refresh)
onMounted(() => {
  void refresh()
  timer = setInterval(() => { if (signingIn.value) void refresh() }, 1500)
})
onUnmounted(() => {
  if (timer !== undefined) clearInterval(timer)
})
</script>

<template>
  <PageHeader title="Integrations" sub="What your agents can reach, and the keys they reach it with.">
    <Button variant="outline" size="sm" class="rounded-lg text-[13px] border-line2" :disabled="pending" @click="act({ action: 'check' })">
      <RefreshCw class="size-3.5" aria-hidden="true" :class="{ 'motion-safe:animate-spin': pendingAction === 'check' }" />
      {{ pendingAction === 'check' ? 'Checking…' : 'Check status' }}
    </Button>
    <Button size="sm" class="rounded-lg text-[13px]" :disabled="pending || signingIn" @click="beginSignin()">
      <LogIn class="size-3.5" aria-hidden="true" /> Sign in to Claude
    </Button>
    <Button variant="outline" size="sm" class="rounded-lg text-[13px] border-line2" :disabled="pending" @click="editToken()"><KeyRound class="size-3.5" aria-hidden="true" /> Connect with a token</Button>
  </PageHeader>
  <IntegrationTabs />

  <LoadState v-if="loadError" :error="loadError" label="accounts" class="mb-5" @retry="refresh" />
  <p v-if="error" role="alert" class="mb-4 text-[14px] text-critical">{{ error }}</p>

  <div v-if="signin && signin.status !== 'complete'" ref="signinSection" tabindex="-1" class="mb-5">
    <Panel pad title="Claude sign-in" :icon="KeyRound">
      <p v-if="signin.error" role="alert" class="text-[14px] text-critical">{{ signin.error }}</p>
      <div v-else class="space-y-3">
        <p v-if="!signin.url" role="status" class="text-[14px] text-ink2">Starting sign-in…</p>
        <template v-else>
          <a :href="signin.url" target="_blank" rel="noopener noreferrer" class="text-[14px] font-medium text-ink underline decoration-line2 underline-offset-4 hover:decoration-ink">
            Open Claude sign-in
          </a>
          <p class="text-[14px] text-ink2">Enter the code below if Claude displays one. If Claude redirects back, this page updates automatically.</p>
          <div class="flex flex-wrap items-end gap-2">
            <label class="flex w-full min-w-0 flex-col gap-1 text-[14px] sm:w-52">Sign-in code
              <input v-model="code" type="password" autocomplete="one-time-code" spellcheck="false" maxlength="2048"
                data-1p-ignore data-lpignore="true" data-bwignore data-protonpass-ignore
                class="h-9 rounded-[10px] border border-line2 bg-panel px-3 text-[13.5px] text-ink placeholder:text-ink3 focus-visible:outline-2 focus-visible:outline-ring" @keydown.enter.prevent="submitCode" />
            </label>
            <Button type="button" size="sm" :disabled="pending || !code.trim()" @click="submitCode">Submit code</Button>
          </div>
        </template>
        <Button variant="ghost" size="sm" :disabled="pending" @click="act({ action: 'cancel', id: signin.id })">Cancel sign-in</Button>
      </div>
    </Panel>
  </div>

  <Panel pad v-if="tokenForm" :title="replaces ? 'Replace Claude token' : 'Connect Claude with a token'" :icon="KeyRound" class="mb-5">
    <p class="mb-3 text-[14px] text-ink2">Run <code>claude setup-token</code> and enter the token below.</p>
    <div class="space-y-3">
      <label class="flex flex-col gap-1 text-[14px]">Connection name
        <input ref="tokenNameInput" v-model="label" maxlength="80" class="h-9 rounded-[10px] border border-line2 bg-panel px-3 text-[13.5px] text-ink placeholder:text-ink3 focus-visible:outline-2 focus-visible:outline-ring" />
      </label>
      <label class="flex flex-col gap-1 text-[14px]">Claude token
        <input v-model="token" type="password" autocomplete="off" spellcheck="false" maxlength="4096"
          data-1p-ignore data-lpignore="true" data-bwignore data-protonpass-ignore
          class="h-9 rounded-[10px] border border-line2 bg-panel px-3 text-[13.5px] text-ink placeholder:text-ink3 focus-visible:outline-2 focus-visible:outline-ring" @keydown.enter.prevent="submitToken" />
      </label>
      <div class="flex gap-2">
        <Button size="sm" type="button" :disabled="pending || !token.trim() || !label.trim()" @click="submitToken"><Check class="size-3.5" aria-hidden="true" /> Save and check</Button>
        <Button size="sm" variant="ghost" type="button" @click="closeTokenForm()"><X class="size-3.5" aria-hidden="true" /> Cancel</Button>
      </div>
    </div>
  </Panel>
  <div class="grid min-w-0 items-start gap-5 @min-[42rem]:grid-cols-[minmax(0,1fr)_minmax(0,1.45fr)]">
    <Panel pad title="Claude accounts" :icon="UserRound" :count="data?.accounts.length">
      <p v-if="!data && !loadError" role="status" class="text-[14px] text-ink2">Loading accounts…</p>
      <ul v-else-if="data" class="space-y-1">
        <li v-for="item in data.accounts" :key="item.id">
          <button type="button" class="flex w-full items-center gap-3 rounded-xl px-3 py-2.5 text-left hover:bg-panel2"
            :class="account?.id === item.id ? 'bg-panel2 ring-1 ring-line2' : ''" :aria-pressed="account?.id === item.id"
            @click="selected = item.id">
            <span class="min-w-0 flex-1 break-all text-[14px] font-medium">{{ accountLabel(item) }}</span>
            <StatusTag :tone="tone(item.status)">{{ statuses[item.status] ?? item.status }}</StatusTag>
          </button>
        </li>
      </ul>
    </Panel>

    <Panel pad v-if="account" :title="accountLabel(account)" :icon="KeyRound">
      <dl class="grid grid-cols-[8rem_1fr] gap-x-4 gap-y-2.5 text-[14px]">
        <dt class="text-ink3">Type</dt><dd>{{ kinds[account.kind] ?? account.kind }}</dd>
        <dt class="text-ink3">Status</dt><dd><StatusTag :tone="tone(account.status)">{{ statuses[account.status] ?? account.status }}</StatusTag></dd>
        <template v-if="account.reason">
          <dt class="text-ink3">Details</dt><dd class="break-words">{{ legacyReasons[account.reason] ?? account.reason }}</dd>
        </template>
        <template v-if="account.quota">
          <dt class="text-ink3">Limit</dt>
          <dd>{{ quotas[account.quota] ?? account.quota }}<template v-if="account.utilization !== null">
            · {{ Math.round(account.utilization * 100) }}% used</template></dd>
        </template>
        <template v-if="account.resets_at">
          <dt class="text-ink3">Resets at</dt><dd>{{ resetTime(account.resets_at) }}</dd>
        </template>
        <template v-else-if="account.retry_at">
          <dt class="text-ink3">Retry after</dt><dd>{{ resetTime(account.retry_at) }}</dd>
        </template>
      </dl>
      <div v-if="account.kind !== 'default'" class="mt-4 flex flex-wrap gap-2">
        <Button v-if="account.kind === 'login'" variant="outline" size="sm" :disabled="pending || signingIn"
          @click="beginSignin(account.id)"><LogIn class="size-3.5" aria-hidden="true" /> Sign in again</Button>
        <Button v-if="account.kind === 'connection'" variant="outline" size="sm" :disabled="pending"
          @click="editToken(account)"><KeyRound class="size-3.5" aria-hidden="true" /> Replace token</Button>
        <Button variant="outline" size="sm" :disabled="pending" @click="remove(account)"><Trash2 class="size-3.5" aria-hidden="true" /> Remove account</Button>
      </div>
    </Panel>
  </div>
</template>
