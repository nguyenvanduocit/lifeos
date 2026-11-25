<script setup lang="ts">
import { nextTick, reactive, ref } from 'vue'
import { chat, resetChat } from '../core/link'

interface Message {
  from: 'me' | 'bot'
  text: string
  tools: string[]
  pending?: boolean
  error?: boolean
}

const emit = defineEmits<{ changed: [] }>()
const messages = ref<Message[]>([])
const input = ref('')
const busy = ref(false)
const log = ref<HTMLElement | null>(null)

const toolName = (name: string) => name.replace(/^mcp__lifeos__/, '')

async function scroll() {
  await nextTick()
  log.value?.scrollTo({ top: log.value.scrollHeight })
}

async function send() {
  const text = input.value.trim()
  if (!text || busy.value) return
  input.value = ''
  busy.value = true
  messages.value.push({ from: 'me', text, tools: [] })
  const reply = reactive<Message>({ from: 'bot', text: '', tools: [], pending: true })
  messages.value.push(reply)
  scroll()
  try {
    const done = await chat(text, (p) => {
      if ('Text' in p) reply.text += p.Text
      else reply.tools.push(toolName(p.Tool))
      scroll()
    })
    reply.text = done.text
    reply.tools = [...new Set(done.tools.map(toolName))]
    emit('changed')
  } catch (err) {
    reply.text = `Lỗi: ${err instanceof Error ? err.message : String(err)}`
    reply.error = true
  } finally {
    reply.pending = false
    busy.value = false
    scroll()
  }
}

async function reset() {
  await resetChat()
  messages.value = []
}

function onKey(e: KeyboardEvent) {
  if (e.key === 'Enter' && !e.shiftKey && !e.isComposing) {
    e.preventDefault()
    send()
  }
}
</script>

<template>
  <section class="card chat">
    <div class="chat-head">
      <h1>Trò chuyện</h1>
      <button class="ghost" :disabled="busy" @click="reset">Cuộc mới</button>
    </div>
    <div ref="log" class="log">
      <div v-for="(m, i) in messages" :key="i" class="msg" :class="[m.from, { error: m.error }]">
        <span>{{ m.text || (m.pending ? '…' : '') }}</span>
        <div v-if="m.tools.length" class="tools">
          {{ m.pending ? 'đang dùng' : 'đã dùng' }}: {{ [...new Set(m.tools)].join(', ') }}
        </div>
      </div>
    </div>
    <form @submit.prevent="send">
      <textarea
        v-model="input"
        placeholder="Hôm nay mình nên làm gì? · Vừa chi 45k ăn trưa · Đã chạy 5km"
        @keydown="onKey"
      />
      <button :disabled="busy || !input.trim()">Gửi</button>
    </form>
  </section>
</template>
