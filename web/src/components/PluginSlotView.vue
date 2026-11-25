<script setup lang="ts">
import { computed, shallowRef, ref, watch } from 'vue'
import { callPlugin, type Plugin, type PluginSlot } from '../plugins'
import { parseCollection, type Collection } from '../pluginCollection'
import PluginFrame from './PluginFrame.vue'
import PluginCollection from './PluginCollection.vue'

import { parseUiDocument } from '../jsonUi'
import type { WidgetUi } from '../core/generated/WidgetUi'
import WidgetUiNode from './WidgetUiNode.vue'

const ui = shallowRef<WidgetUi>()
const props = defineProps<{ plugin: Plugin; slot: PluginSlot }>()
const path = computed(() => props.slot.expose.endsWith('.json') ?
  `/plugin-assets/${encodeURIComponent(props.plugin.id)}/${props.slot.expose.split('/').map(encodeURIComponent).join('/')}` : '')
const schema = shallowRef<Collection>()
const error = ref('')
watch(path, async (url, _, cleanup) => {
  const abort = new AbortController()
  cleanup(() => abort.abort())
  ui.value = undefined
  schema.value = undefined
  error.value = ''
  if (!url) return
  try {
    const res = await fetch(url, { signal: abort.signal, cache: 'no-store' })
    if (!res.ok) throw new Error(`Plugin UI: HTTP ${res.status}`)
    let document = await res.json()
    if (document?.kind === 'ui_source' && !abort.signal.aborted) {
      if (!props.plugin.functions.some(f => f.name === document.tool && f.effect === 'read'))
        throw Error('UI source must be an enabled read function')
      document = JSON.parse(await callPlugin(props.plugin, document.tool, document.arguments ?? {}))
      if (document?.kind !== 'ui') throw Error('Plugin function must return a UI document')
    }
    if (abort.signal.aborted) return
    if (document?.kind === 'ui') ui.value = parseUiDocument(document)
    else schema.value = parseCollection(document)
  } catch (e) {
    if (!abort.signal.aborted) error.value = String(e)
  }
}, { immediate: true })
</script>
<template>
  <PluginFrame v-if="!path" :plugin="plugin" :slot="slot" />
  <WidgetUiNode v-else-if="ui" :key="path" :element-id="ui.root" :elements="ui.elements" :widget="{ id: plugin.id, page: $route.path }" />
  <PluginCollection v-else-if="schema" :key="path" :schema="schema" :plugin="plugin" :slot="slot" />
  <p v-else-if="error" role="alert">{{ error }}</p>
  <p v-else role="status">Loading…</p>
</template>
