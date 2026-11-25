<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import { CalendarDays, ChevronRight, KeyRound, LogIn, LogOut, Mail, Settings2, UserRound, Users } from '@lucide/vue'
import AskChip from '../components/AskChip.vue'
import PageHeader from '../components/ds/PageHeader.vue'
import Panel from '../components/ds/Panel.vue'
import StatusTag from '../components/ds/StatusTag.vue'
import LoadState from '../components/LoadState.vue'
import { Button } from '../components/ui/button'
import type { GoogleView } from '../core/generated/GoogleView'
import { revision } from '../core/link'
import { useRead } from '../data'

const data = ref<GoogleView>()
const pendingAction = ref<string>()
const pending = computed(() => pendingAction.value !== undefined)
const error = ref('')
const loadError = ref('')
const clientJson = ref('')
const returned = ref('')
const editingClient = ref(false)
const signin = computed(() => data.value?.signin)
const waiting = computed(() => signin.value?.status === 'waiting')
const showClientForm = computed(() => !data.value?.client || editingClient.value)
// A desktop client only comes back to loopback; opened at a domain, Google needs a web client with this redirect.
const loopback = computed(() => /^http:\/\/(127\.0\.0\.1|\[::1\])[:/]/.test(data.value?.redirect ?? ''))
const clientMismatch = computed(() => data.value?.client?.kind === 'desktop' && !loopback.value)

const { data: crew } = useRead('list_crew')
const names = computed(() => new Map((crew.value ?? []).map((m) => [m.id, m.name])))

const services: Record<string, string> = { gmail: 'Gmail', calendar: 'Calendar', drive: 'Drive', contacts: 'Contacts' }
const missing = computed(() =>
  Object.keys(services).filter((s) => data.value?.account && !data.value.account.services.includes(s)))

let request = 0
let timer: ReturnType<typeof setInterval> | undefined

async function send(payload?: object) {
  const number = ++request
  const response = await fetch('/api/google', {
    credentials: 'same-origin',
    cache: 'no-store',
    headers: { 'X-LifeOS-Account': '1', ...(payload ? { 'Content-Type': 'application/json' } : {}) },
    ...(payload ? { method: 'POST', body: JSON.stringify(payload) } : {}),
  })
  if (!response.headers.get('content-type')?.includes('application/json')) {
    throw new Error('Could not reach the Google app.')
  }
  const result = await response.json()
  if (!response.ok) throw new Error(result.error ?? 'Google request failed.')
  if (number === request) data.value = result
}

async function refresh() {
  if (pending.value) return
  try {
    await send()
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
    await send(payload)
    return true
  } catch (err) {
    error.value = err instanceof Error ? err.message : String(err)
    return false
  } finally {
    pendingAction.value = undefined
  }
}

async function signIn() {
  // Open the tab while the click still counts, so no popup blocker stops it; Google's page goes in once known.
  const tab = window.open('about:blank', '_blank')
  if (await act({ action: 'login' }) && signin.value?.url) {
    if (tab) {
      tab.opener = null
      tab.location.href = signin.value.url
    }
  } else {
    tab?.close()
  }
}

async function saveClient() {
  if (await act({ action: 'client', json: clientJson.value })) {
    clientJson.value = ''
    editingClient.value = false
  }
}

function submitReturned() {
  const value = returned.value.trim()
  returned.value = ''
  if (value) void act({ action: 'code', returned: value })
}

function signOut() {
  if (window.confirm(`Sign ${data.value?.account?.email ?? 'Google'} out of LifeOS? Agents lose access to Google.`)) {
    void act({ action: 'logout' })
  }
}

function forgetClient() {
  if (window.confirm('Remove the OAuth client? You will be signed out of Google too.')) void act({ action: 'forget_client' })
}

function since(text: string) {
  return new Intl.DateTimeFormat('en-US', { dateStyle: 'medium', timeStyle: 'short' }).format(new Date(text))
}

watch(revision, refresh)
onMounted(() => {
  void refresh()
  timer = setInterval(() => { if (waiting.value) void refresh() }, 1500)
})
onUnmounted(() => {
  if (timer !== undefined) clearInterval(timer)
})
</script>

<template>
  <nav aria-label="Breadcrumb" class="mb-4 flex items-center gap-1.5 text-[14px] text-ink2">
    <RouterLink to="/connections" class="hover:text-ink">Integrations</RouterLink>
    <ChevronRight class="size-3.5 text-ink3" aria-hidden="true" />
    <span class="text-ink" aria-current="page">Google</span>
  </nav>
  <PageHeader title="Google" sub="Gmail, Calendar, Drive and Contacts for your agents, signed in with your own Google account.">
    <Button v-if="data?.client && !data.account" size="sm" class="rounded-lg text-[13px]" :disabled="pending || waiting" @click="signIn">
      <LogIn class="size-3.5" aria-hidden="true" /> Sign in with Google
    </Button>
  </PageHeader>

  <LoadState v-if="loadError" :error="loadError" label="Google" class="mb-4" @retry="refresh" />
  <p v-if="error" role="alert" class="mb-4 text-[14px] text-critical">{{ error }}</p>

  <Panel v-if="signin" pad title="Google sign-in" :icon="KeyRound" class="mb-5">
    <div v-if="signin.status === 'failed'" class="space-y-3">
      <p role="alert" class="text-[14px] text-critical">{{ signin.error }}</p>
      <div class="flex gap-2">
        <Button size="sm" :disabled="pending" @click="signIn">Try again</Button>
        <Button size="sm" variant="ghost" :disabled="pending" @click="act({ action: 'cancel' })">Dismiss</Button>
      </div>
    </div>
    <div v-else class="space-y-3">
      <p role="status" class="text-[14px] text-ink2">
        Finish signing in on Google's page.
        <a :href="signin.url" target="_blank" rel="noopener noreferrer" class="font-medium text-ink underline decoration-line2 underline-offset-4 hover:decoration-ink">Open it again</a>
        if no tab opened. This page updates when you are done.
      </p>
      <details class="text-[14px]">
        <summary class="cursor-pointer text-ink2 hover:text-ink">The browser ended on a page that could not load?</summary>
        <p class="mt-2 text-[12.5px] text-ink2">
          Google sends you back to <code class="break-all">{{ data?.redirect }}</code>. If that page could not load,
          copy the whole address from that tab and paste it here.
        </p>
        <div class="mt-2 flex flex-wrap items-end gap-2">
          <input v-model="returned" type="url" autocomplete="off" spellcheck="false" maxlength="8192" placeholder="http://127.0.0.1:…/api/google/callback?…"
            data-1p-ignore data-lpignore="true" data-bwignore data-protonpass-ignore
            class="min-w-0 flex-1 h-9 rounded-[10px] border border-line2 bg-panel px-3 text-[13.5px] text-ink placeholder:text-ink3 focus-visible:outline-2 focus-visible:outline-ring" @keydown.enter.prevent="submitReturned" />
          <Button size="sm" :disabled="pending || !returned.trim()" @click="submitReturned">Finish sign-in</Button>
        </div>
      </details>
      <Button variant="ghost" size="sm" :disabled="pending" @click="act({ action: 'cancel' })">Cancel sign-in</Button>
    </div>
  </Panel>

  <div v-if="data" class="grid min-w-0 items-start gap-5 @min-[58rem]:grid-cols-2">
    <Panel pad title="Account" :icon="UserRound">
      <template v-if="data.account">
        <dl class="grid grid-cols-[8rem_1fr] gap-x-4 gap-y-2.5 text-[14px]">
          <dt class="text-ink3">Signed in as</dt><dd class="break-all font-semibold">{{ data.account.email }}</dd>
          <dt class="text-ink3">Services</dt>
          <dd class="flex flex-wrap gap-1.5">
            <StatusTag v-for="s in data.account.services" :key="s" tone="good">{{ services[s] ?? s }}</StatusTag>
          </dd>
          <dt class="text-ink3">Since</dt><dd>{{ since(data.account.since) }}</dd>
          <dt class="text-ink3">Access</dt>
          <dd>Assistant<template v-for="id in data.members" :key="id">, {{ names.get(id) ?? id }}</template></dd>
        </dl>
        <p v-if="missing.length" class="mt-3 text-[14px] text-ink2">
          Not allowed: {{ missing.map((s) => services[s]).join(', ') }}. Sign in again and tick them to let agents use them.
        </p>
        <div class="mt-4 flex flex-wrap gap-2">
          <Button variant="outline" size="sm" :disabled="pending || waiting" @click="signIn">Sign in again</Button>
          <Button variant="outline" size="sm" :disabled="pending" @click="signOut"><LogOut class="size-4" /> Sign out</Button>
        </div>
        <div class="mt-3 flex flex-wrap gap-1.5">
          <AskChip :icon="Users" label="Edit crew access" words="Change crew access to the google connection: " />
          <AskChip :icon="Mail" label="What's new in my mail?" words="Check my Gmail inbox and tell me what needs my attention." />
          <AskChip :icon="CalendarDays" label="My week" words="Read my Google Calendar for this week and summarize it." />
        </div>
      </template>
      <div v-else class="space-y-3 text-[14px] text-ink2">
        <p>Not signed in. Agents cannot use Google until you sign in.</p>
        <Button v-if="data.client" size="sm" :disabled="pending || waiting" @click="signIn">
          <LogIn class="size-3.5" aria-hidden="true" /> Sign in with Google
        </Button>
        <p v-else>Add your OAuth client first (one time).</p>
      </div>
    </Panel>

    <Panel pad title="OAuth client" :icon="Settings2">
      <template v-if="data.client && !editingClient">
        <dl class="grid grid-cols-[8rem_1fr] gap-x-4 gap-y-2.5 text-[14px]">
          <dt class="text-ink3">Client ID</dt><dd class="break-all font-mono text-[12.5px]">{{ data.client.client_id }}</dd>
          <dt class="text-ink3">Type</dt><dd>{{ data.client.kind === 'web' ? 'Web application' : 'Desktop app' }}</dd>
          <dt class="text-ink3">Redirect URI</dt><dd class="break-all font-mono text-[12.5px]">{{ data.redirect }}</dd>
          <template v-if="data.client.project"><dt class="text-ink3">Project</dt><dd>{{ data.client.project }}</dd></template>
        </dl>
        <div class="mt-4 flex flex-wrap gap-2">
          <Button variant="outline" size="sm" :disabled="pending" @click="editingClient = true">Replace client</Button>
          <Button variant="outline" size="sm" :disabled="pending" @click="forgetClient">Remove client</Button>
        </div>
        <p v-if="clientMismatch" role="alert" class="mt-3 text-[14px] text-critical">
          This is a Desktop app client, which Google only sends back to 127.0.0.1. Opened at this address, use a
          Web application client with the redirect URI above.
        </p>
        <p v-else-if="data.client.kind === 'web'" class="mt-3 text-[12.5px] text-ink2">
          The redirect URI above must be listed under Authorized redirect URIs of this client in Google Cloud.
        </p>
      </template>
      <div v-if="showClientForm" class="space-y-3 text-[14px]">
        <p class="text-ink2">LifeOS signs in with an OAuth client in your own Google Cloud project, so your data goes only between this machine and Google. Set it up once:</p>
        <ol class="list-decimal space-y-1.5 pl-5 text-ink2">
          <li><a class="font-medium text-ink underline decoration-line2 underline-offset-4 hover:decoration-ink" href="https://console.cloud.google.com/projectcreate" target="_blank" rel="noopener noreferrer">Create a Google Cloud project</a>.</li>
          <li>Enable the
            <a class="font-medium text-ink underline decoration-line2 underline-offset-4 hover:decoration-ink" href="https://console.cloud.google.com/apis/library/gmail.googleapis.com" target="_blank" rel="noopener noreferrer">Gmail</a>,
            <a class="font-medium text-ink underline decoration-line2 underline-offset-4 hover:decoration-ink" href="https://console.cloud.google.com/apis/library/calendar-json.googleapis.com" target="_blank" rel="noopener noreferrer">Calendar</a>,
            <a class="font-medium text-ink underline decoration-line2 underline-offset-4 hover:decoration-ink" href="https://console.cloud.google.com/apis/library/drive.googleapis.com" target="_blank" rel="noopener noreferrer">Drive</a> and
            <a class="font-medium text-ink underline decoration-line2 underline-offset-4 hover:decoration-ink" href="https://console.cloud.google.com/apis/library/people.googleapis.com" target="_blank" rel="noopener noreferrer">People</a> APIs.</li>
          <li>In <a class="font-medium text-ink underline decoration-line2 underline-offset-4 hover:decoration-ink" href="https://console.cloud.google.com/auth/overview" target="_blank" rel="noopener noreferrer">Google Auth Platform</a>, set up the app (audience External, or Internal for a Workspace account), then under Audience choose <strong>Publish app</strong>. An app left in Testing makes you sign in again every 7 days. Google will say the app is unverified; that is expected for your own app.</li>
          <li v-if="loopback">Under <a class="font-medium text-ink underline decoration-line2 underline-offset-4 hover:decoration-ink" href="https://console.cloud.google.com/auth/clients" target="_blank" rel="noopener noreferrer">Clients</a>, create a client of type <strong>Desktop app</strong> and download its JSON.</li>
          <li v-else>Under <a class="font-medium text-ink underline decoration-line2 underline-offset-4 hover:decoration-ink" href="https://console.cloud.google.com/auth/clients" target="_blank" rel="noopener noreferrer">Clients</a>, create a client of type <strong>Web application</strong>, add
            <code class="break-all rounded bg-panel2 px-1">{{ data.redirect }}</code> under <strong>Authorized redirect URIs</strong>, and download its JSON.</li>
          <li>Paste the JSON below.</li>
        </ol>
        <textarea v-model="clientJson" rows="5" spellcheck="false" autocomplete="off" maxlength="16384"
          placeholder='{"installed":{"client_id":"….apps.googleusercontent.com", …}}'
          data-1p-ignore data-lpignore="true" data-bwignore data-protonpass-ignore
          class="w-full rounded-[10px] border border-line2 bg-panel px-3 py-2 font-mono text-[12.5px] text-ink placeholder:text-ink3 focus-visible:outline-2 focus-visible:outline-ring" />
        <div class="flex gap-2">
          <Button size="sm" :disabled="pending || !clientJson.trim()" @click="saveClient">Save client</Button>
          <Button v-if="editingClient" size="sm" variant="ghost" @click="editingClient = false; clientJson = ''">Cancel</Button>
        </div>
      </div>
    </Panel>
  </div>
</template>
