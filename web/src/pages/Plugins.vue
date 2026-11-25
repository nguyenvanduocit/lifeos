<script setup lang="ts">
import { ref } from 'vue'
import { Download, LoaderCircle, Puzzle, Settings2 } from '@lucide/vue'
import IntegrationTabs from '../components/IntegrationTabs.vue'
import LoadState from '../components/LoadState.vue'
import PageHeader from '../components/ds/PageHeader.vue'
import StatusTag from '../components/ds/StatusTag.vue'
import PluginFrame from '../components/PluginFrame.vue'
import { Button } from '../components/ui/button'
import { availablePlugins, loadPlugins, managePlugin, pluginError, plugins, pluginsLoading } from '../plugins'

const busy = ref<string>()
const settings = ref<string>()
const actionError = ref<string>()

async function change(id: string, action: Parameters<typeof managePlugin>[1]) {
  busy.value = id
  actionError.value = undefined
  try {
    await managePlugin(id, action)
    if (action === 'disable' && settings.value === id) settings.value = undefined
  } catch (error) {
    actionError.value = error instanceof Error ? error.message : String(error)
  } finally {
    busy.value = undefined
  }
}

function description(id: string) {
  return id === 'github' ? 'GitHub MCP access and signed repository events.' : ''
}
</script>

<template>
  <PageHeader title="Integrations" />
  <IntegrationTabs />
  <div class="max-w-[960px] space-y-6">
    <LoadState :loading="pluginsLoading && !plugins.length" :error="pluginError ?? undefined" label="plugins" @retry="loadPlugins" />
    <p v-if="actionError" role="alert" class="rounded-xl border border-critical/25 bg-critical-wash px-4 py-3 text-sm text-critical">{{ actionError }}</p>

    <section v-if="!pluginsLoading || plugins.length" aria-labelledby="installed-plugins">
      <h2 id="installed-plugins" class="mb-3 text-[17px] font-semibold">Installed plugins</h2>
      <p v-if="!plugins.length && !pluginError" class="text-sm text-ink2">No plugins installed.</p>
      <ul v-else-if="plugins.length" class="divide-y divide-line rounded-2xl border border-line bg-panel">
        <li v-for="plugin in plugins" :key="plugin.id">
          <div class="flex flex-wrap items-center gap-3 p-4">
            <span class="grid size-10 shrink-0 place-items-center rounded-xl bg-panel2 text-ink2"><Puzzle class="size-5" aria-hidden="true" /></span>
            <div class="min-w-40 flex-1">
              <div class="flex flex-wrap items-center gap-2">
                <h3 class="font-medium">{{ plugin.label }}</h3>
                <StatusTag :tone="plugin.error ? 'warning' : plugin.enabled ? 'good' : 'neutral'">
                  {{ plugin.error ? 'Error' : plugin.enabled ? 'Enabled' : 'Disabled' }}
                </StatusTag>
              </div>
              <p v-if="description(plugin.id)" class="mt-1 text-sm text-ink2">{{ description(plugin.id) }}</p>
            </div>
            <Button v-if="plugin.enabled && (plugin.functions.length || plugin.slots.some((slot) => slot.slot === 'page'))"
              variant="ghost" size="sm" :aria-expanded="settings === plugin.id"
              @click="settings = settings === plugin.id ? undefined : plugin.id">
              <Settings2 class="size-4" /> Details
            </Button>
            <Button variant="outline" size="sm" :disabled="!!busy" @click="change(plugin.id, plugin.enabled ? 'disable' : 'enable')">
              <LoaderCircle v-if="busy === plugin.id" class="size-4 animate-spin" />
              {{ plugin.enabled ? 'Disable' : 'Enable' }}
            </Button>
          </div>
          <p v-if="plugin.retrying_deliveries" role="status" class="px-4 pb-4 text-sm text-ink2">
            {{ plugin.retrying_deliveries }} event {{ plugin.retrying_deliveries === 1 ? 'delivery is' : 'deliveries are' }} waiting to retry.
          </p>
          <p v-if="plugin.error" role="alert" class="px-4 pb-4 text-sm text-critical">{{ plugin.error }}</p>
          <div v-if="settings === plugin.id && plugin.enabled" class="space-y-3 border-t border-line p-4">
            <ul v-if="plugin.functions.length" class="space-y-1 text-sm text-ink2">
              <li v-for="fn in plugin.functions" :key="fn.name"><code>{{ fn.name }}</code> · {{ fn.description }}</li>
            </ul>
            <PluginFrame v-for="slot in plugin.slots.filter((s) => s.slot === 'page')" :key="slot.id" :plugin="plugin" :slot="slot" />
          </div>
        </li>
      </ul>
    </section>

    <section v-if="availablePlugins.length" aria-labelledby="available-plugins">
      <h2 id="available-plugins" class="mb-3 text-[17px] font-semibold">Available to install</h2>
      <ul class="divide-y divide-line rounded-2xl border border-line bg-panel">
        <li v-for="plugin in availablePlugins" :key="plugin.id" class="flex flex-wrap items-center gap-3 p-4">
          <span class="grid size-10 shrink-0 place-items-center rounded-xl bg-panel2 text-ink2"><Puzzle class="size-5" aria-hidden="true" /></span>
          <div class="min-w-40 flex-1">
            <h3 class="font-medium">{{ plugin.label }}</h3>
            <p v-if="description(plugin.id)" class="mt-1 text-sm text-ink2">{{ description(plugin.id) }}</p>
          </div>
          <Button size="sm" :disabled="!!busy" @click="change(plugin.id, 'install')">
            <LoaderCircle v-if="busy === plugin.id" class="size-4 animate-spin" />
            <Download v-else class="size-4" /> Install
          </Button>
        </li>
      </ul>
    </section>
  </div>
</template>
