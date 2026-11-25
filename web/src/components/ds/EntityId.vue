<script setup lang="ts">
import { Check, Copy } from '@lucide/vue'
import { ref } from 'vue'

const props = defineProps<{ value: string }>()
const status = ref('')

async function copy() {
  try {
    await navigator.clipboard.writeText(props.value)
    status.value = 'Copied'
  } catch {
    status.value = 'Could not copy'
  }
}
</script>

<template>
  <button type="button" class="inline-flex max-w-full items-center gap-1.5 rounded-md px-1.5 py-1 text-xs text-ink2 hover:bg-panel3 hover:text-ink"
    :aria-label="'Copy ID ' + value" :title="status || 'Copy ID'" @click="copy">
    <code class="truncate font-mono">{{ value }}</code>
    <component :is="status === 'Copied' ? Check : Copy" class="size-3 shrink-0" aria-hidden="true" />
    <span class="sr-only" role="status">{{ status }}</span>
  </button>
</template>
