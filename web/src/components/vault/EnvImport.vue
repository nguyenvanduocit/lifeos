<script setup lang="ts">
// Keys from a .env file (agentes.chat ADR 0144 §11): the owner drops, picks or pastes one; it is read here in the
// browser, each key reviewed (new, already saved and left unless ticked to replace, or one that can't be a key), then
// all saved in one request with who may use them. Values stay behind dots and the text is dropped when it closes.
import { computed, ref } from 'vue'
import { FileUp, X } from '@lucide/vue'
import { Button } from '../ui/button'
import StatusTag from '../ds/StatusTag.vue'
import AgentAccess from './AgentAccess.vue'
import { importVaultKeys, keyProblem, parseEnv } from '../../vault'

const props = defineProps<{ saved: string[] }>()
const emit = defineEmits<{ close: []; done: [] }>()
const text = ref('')
const replace = ref<string[]>([])
const access = ref<{ all_agents: boolean; agents: string[] }>({ all_agents: false, agents: ['assistant'] })
const sending = ref(false)
const error = ref('')
const dragging = ref(false)

const saved = computed(() => new Set(props.saved))
const parsed = computed(() => parseEnv(text.value))
const rows = computed(() =>
  parsed.value.keys.map((k) => {
    const problem = keyProblem(k.name, k.value)
    return { ...k, problem, exists: !problem && saved.value.has(k.name) }
  }),
)
const sendable = computed(() => rows.value.filter((r) => !r.problem && (!r.exists || replace.value.includes(r.name))))
const tooBig = computed(() => text.value.length > 512 * 1024 || rows.value.length > 200)

async function readFile(file: File | undefined) {
  if (!file) return
  if (file.size > 512 * 1024) { error.value = 'The file is over 512 KB.'; return }
  error.value = ''
  text.value = await file.text()
}

async function save() {
  if (!sendable.value.length || sending.value) return
  sending.value = true
  error.value = ''
  try {
    await importVaultKeys({
      keys: sendable.value.map((r) => ({ name: r.name, value: r.value })),
      replace: replace.value,
      all_agents: access.value.all_agents,
      agents: access.value.agents,
    })
    text.value = ''
    emit('done')
  } catch (err) {
    error.value = err instanceof Error ? err.message : String(err)
  } finally {
    sending.value = false
  }
}

function close() {
  text.value = ''
  emit('close')
}
</script>

<template>
  <div class="px-5 py-4">
    <div class="mb-3 flex items-center gap-2">
      <FileUp class="size-4 text-ink2" aria-hidden="true" />
      <h3 class="text-[15px] font-semibold">Import .env</h3>
      <button type="button" class="ml-auto grid size-8 place-items-center rounded-lg text-ink2 hover:bg-panel2 hover:text-ink" aria-label="Close" @click="close"><X class="size-4" /></button>
    </div>

    <label v-if="!rows.length"
      class="flex cursor-pointer flex-col items-center gap-2 rounded-xl border border-dashed px-4 py-8 text-center transition-colors"
      :class="dragging ? 'border-ink bg-panel2' : 'border-line2 hover:bg-panel2/60'"
      @dragover.prevent="dragging = true" @dragleave="dragging = false"
      @drop.prevent="dragging = false; readFile($event.dataTransfer?.files[0])">
      <FileUp class="size-6 text-ink3" aria-hidden="true" />
      <span class="text-[14px] font-medium">Drop a .env file here, or choose one</span>
      <span class="text-[13px] text-ink2">Read in your browser. Only the keys you save leave it.</span>
      <input type="file" class="sr-only" accept=".env,.json,text/plain" @change="readFile(($event.target as HTMLInputElement).files?.[0])" />
    </label>
    <textarea v-if="!rows.length" v-model="text" rows="4" spellcheck="false" autocomplete="off" placeholder="…or paste it: GITHUB_TOKEN=…"
      class="mt-3 w-full rounded-xl border border-line2 bg-panel px-3 py-2 font-mono text-[12.5px] text-ink placeholder:text-ink3 focus-visible:outline-2 focus-visible:outline-ring" />
    <p v-if="text.trim() && !rows.length" class="mt-2 text-[13px] text-critical">No NAME=value line in this text.</p>

    <template v-if="rows.length">
      <p v-if="tooBig" role="alert" class="mb-2 text-[13px] text-critical">At most 200 keys and 512 KB at a time.</p>
      <p v-if="parsed.twice.length" class="mb-2 text-[13px] text-ink2">Written twice, the last value kept: {{ parsed.twice.join(', ') }}</p>
      <ul class="overflow-hidden rounded-xl border border-line">
        <li v-for="r in rows" :key="r.name" class="flex flex-wrap items-center gap-x-3 gap-y-1 border-b border-line px-3.5 py-2.5 last:border-b-0">
          <code class="min-w-0 flex-1 truncate font-mono text-[12.5px]">{{ r.name }}</code>
          <span class="font-mono text-[12.5px] text-ink3" aria-label="value hidden">••••••••</span>
          <StatusTag v-if="r.problem" tone="critical">{{ r.problem }}</StatusTag>
          <label v-else-if="r.exists" class="flex items-center gap-1.5 text-[13px] text-ink2">
            <input v-model="replace" type="checkbox" :value="r.name" class="size-4 accent-[var(--ink)]" /> Already saved · replace
          </label>
          <StatusTag v-else tone="good">New</StatusTag>
        </li>
      </ul>
      <div class="mt-4">
        <p class="mb-2 text-[13px] font-medium text-ink2">Who may use them</p>
        <AgentAccess v-model="access" />
      </div>
      <p v-if="error" role="alert" class="mt-3 text-[13px] text-critical">{{ error }}</p>
      <div class="mt-4 flex flex-wrap items-center gap-2">
        <Button class="h-10 rounded-xl px-4" :disabled="!sendable.length || sending || tooBig" @click="save">
          Save {{ sendable.length }} key{{ sendable.length === 1 ? '' : 's' }}
        </Button>
        <Button variant="ghost" class="h-10 rounded-xl px-4" @click="text = ''; replace = []">Choose another</Button>
      </div>
    </template>
  </div>
</template>
