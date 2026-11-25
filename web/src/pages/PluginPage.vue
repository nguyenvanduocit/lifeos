<script setup lang="ts">
import { computed } from 'vue'
import { useRoute } from 'vue-router'
import PluginSlotView from '../components/PluginSlotView.vue'
import { plugins, pluginsLoading, pluginError } from '../plugins'

const route = useRoute()
const page = computed(() => {
  const plugin = plugins.value.find(p => p.enabled && p.id === route.params.plugin)
  const slot = plugin?.slots.find(s => (s.slot === 'sidebar' || s.slot === 'page') && s.id === route.params.slot)
  return plugin && slot ? { plugin, slot } : undefined
})
</script>

<template>
  <PluginSlotView v-if="page" :plugin="page.plugin" :slot="page.slot" />
  <p v-else role="status">{{ pluginsLoading ? 'Loading plugin…' : pluginError || 'Plugin unavailable.' }}</p>
</template>
