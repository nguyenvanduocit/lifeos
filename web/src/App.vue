<script setup lang="ts">
import { ref } from 'vue'
import TodayPanel from './components/TodayPanel.vue'
import ChatPanel from './components/ChatPanel.vue'
import { linkState } from './core/link'

const todayPanel = ref<InstanceType<typeof TodayPanel> | null>(null)
</script>

<template>
  <header>
    <strong>lifeos</strong>
    <span class="link" :class="linkState.status" :title="linkState.error">
      {{ linkState.status === 'open' ? 'đã kết nối' : linkState.status === 'connecting' ? 'đang kết nối…' : 'mất kết nối' }}
    </span>
  </header>
  <main>
    <TodayPanel ref="todayPanel" />
    <ChatPanel @changed="todayPanel?.refresh()" />
  </main>
</template>
