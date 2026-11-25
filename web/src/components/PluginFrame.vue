<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useEventListener } from '@vueuse/core'
import { revision } from '../core/link'
import { callPlugin, type Plugin, type PluginSlot } from '../plugins'

const props = defineProps<{ plugin: Plugin; slot: PluginSlot }>()
const frame = ref<HTMLIFrameElement>()
const height = ref<number>()
const path = computed(() => `/plugin-assets/${encodeURIComponent(props.plugin.id)}/${props.slot.expose.split('/').map(encodeURIComponent).join('/')}`)

function sendEvent(name: string) {
  frame.value?.contentWindow?.postMessage({ type: 'lifeos:event', name, revision: revision.value }, '*')
}

watch(revision, () => sendEvent('lifeos.changed'))
watch(path, () => { height.value = undefined })

useEventListener(window, 'message', async (event: MessageEvent) => {
  const source = frame.value?.contentWindow
  if (!source || source !== event.source || event.origin !== 'null') return
  const data = event.data
  if (data?.type === 'lifeos:resize' && Number.isFinite(data.height) && data.height > 0) {
    height.value = Math.min(1600, Math.ceil(data.height)) + 2
    return
  }
  if (data?.type !== 'lifeos:call' || typeof data.id !== 'string' || typeof data.name !== 'string') return
  try {
    const result = await callPlugin(props.plugin, data.name, data.arguments)
    source.postMessage({ type: 'lifeos:answer', id: data.id, result }, '*')
  } catch (error) {
    source.postMessage({ type: 'lifeos:answer', id: data.id, error: String(error) }, '*')
  }
})
</script>

<template>
  <iframe
    ref="frame"
    :key="path"
    :src="path"
    :title="slot.title || plugin.label"
    sandbox="allow-scripts"
    class="block w-full rounded-xl border border-line bg-panel"
    :style="height === undefined ? undefined : { height: `${height}px` }"
    :class="height !== undefined ? undefined : slot.slot === 'page' ? 'min-h-[65vh]' : 'min-h-44'"
    @load="sendEvent('lifeos.ready')"
  />
</template>
