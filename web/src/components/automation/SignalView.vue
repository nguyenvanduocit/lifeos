<script setup lang="ts">
import { onMounted, ref } from 'vue'
import type { Routine } from '../../core/generated/Routine'
import LoadState from '../LoadState.vue'

defineProps<{ routines?: Routine[] | null; routinesError?: string | null }>()

interface Subscription { event: string; processor: string; plugin: string; enabled: boolean }
interface SignalEvent { id: number; event: string; at: number; subscribers: number }
const data = ref<{ routineEvents: string[]; signals: { event: string; note: string }[]; subscriptions: Subscription[]; events: SignalEvent[]; error?: string }>()
const error = ref('')
const loading = ref(false)

function source(event: string) {
  const [namespace, id] = event.split('.')
  if (namespace === 'plugin' && id) return 'Plugin · ' + id
  return namespace === 'lifeos' ? 'Core' : 'Unknown source'
}

async function refresh() {
  loading.value = true
  error.value = ''
  try {
    const response = await fetch('/api/signals', { cache: 'no-store' })
    if (!response.ok) throw new Error('Could not load signals')
    data.value = await response.json()
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e)
  } finally {
    loading.value = false
  }
}
onMounted(refresh)
</script>

<template>
  <div class="border-t border-line px-5 py-4 text-sm">
    <div class="flex items-center justify-between gap-3">
      <h2 class="font-semibold">Signal catalog</h2>
      <button class="rounded-lg border border-line2 px-3 py-1 hover:bg-panel2" :disabled="loading" @click="refresh">Refresh</button>
    </div>
    <LoadState v-if="error || !data" :loading="loading" :error="error" label="signals" @retry="refresh" />
    <template v-else>
      <h3 class="mt-4 font-medium">Routine events ({{ data.routineEvents.length }})</h3>
      <p class="py-2 text-ink2">Use these keys in a routine’s <code>on</code> list to run it after a successful user-initiated write.</p>
      <ul class="flex flex-wrap gap-2">
        <li v-for="key in data.routineEvents" :key="key" class="rounded-md border border-line px-2 py-1"><code class="select-all">{{ key }}</code></li>
      </ul>
      <h3 class="mt-4 border-t border-line pt-4 font-medium">Plugin signals ({{ data.signals.length }})</h3>
      <p class="py-2 text-ink2">To subscribe, map a signal key to a processor in the receiving plugin’s <code>events</code> configuration. Plugins can also emit custom keys at runtime.</p>
      <ul class="divide-y divide-line">
        <li v-for="signal in data.signals" :key="signal.event" class="py-2">
          <code class="break-all select-all">{{ signal.event }}</code>
          <p class="text-ink2">{{ source(signal.event) }} · {{ signal.note }}</p>
        </li>
      </ul>
      <h2 class="mt-4 border-t border-line pt-4 font-semibold">Routine subscriptions</h2>
      <p v-if="routinesError || !routines" class="py-3 text-ink2">{{ routinesError || 'Loading routines…' }}</p>
      <ul v-else-if="routines.some(r => r.on.length)" class="divide-y divide-line">
        <template v-for="routine in routines" :key="routine.id">
          <li v-for="event in routine.on" :key="event" class="py-2">
            <code>{{ event }}</code> → {{ routine.title }} · {{ routine.status }}
          </li>
        </template>
      </ul>
      <p v-else class="py-3 text-ink2">No routines subscribe to tool events.</p>
      <h2 class="mt-4 border-t border-line pt-4 font-semibold">Plugin subscriptions ({{ data.subscriptions.length }})</h2>
      <p v-if="data.error" class="py-3 text-critical">{{ data.error }}</p>
      <ul v-else-if="data.subscriptions.length" class="divide-y divide-line">
        <li v-for="s in data.subscriptions" :key="s.plugin + '/' + s.event" class="grid gap-1 py-3">
          <span class="break-all font-mono">{{ s.event }}</span>
          <span class="text-ink2">Source · {{ source(s.event) }} → Plugin · {{ s.plugin }} / {{ s.processor }} · {{ s.enabled ? 'Enabled' : 'Disabled' }}</span>
        </li>
      </ul>
      <p v-else class="py-3 text-ink2">No plugin subscriptions configured.</p>
      <h2 class="mt-4 border-t border-line pt-4 font-semibold">Recent plugin events ({{ data.events.length }})</h2>
      <ul v-if="data.events.length" class="divide-y divide-line">
        <li v-for="e in data.events" :key="e.id" class="flex flex-wrap items-baseline justify-between gap-x-4 py-2">
          <div><span class="break-all font-mono">{{ e.event }}</span>
            <p class="text-ink2">{{ source(e.event) }} · {{ e.subscribers }} {{ e.subscribers === 1 ? 'subscriber' : 'subscribers' }}</p>
          </div>
          <time class="text-ink2" :datetime="new Date(e.at).toISOString()">{{ new Date(e.at).toLocaleString() }}</time>
        </li>
      </ul>
      <p v-else class="py-3 text-ink2">No signal events recorded yet.</p>
    </template>
  </div>
</template>
