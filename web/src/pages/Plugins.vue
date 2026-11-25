<script setup lang="ts">
import { ref } from 'vue'
import { ChevronDown, Download, LoaderCircle, Puzzle } from '@lucide/vue'
import AskChip from '../components/AskChip.vue'
import PluginDetails from '../components/PluginDetails.vue'
import IntegrationTabs from '../components/IntegrationTabs.vue'
import LoadState from '../components/LoadState.vue'
import PageHeader from '../components/ds/PageHeader.vue'
import StatusTag from '../components/ds/StatusTag.vue'
import { Button } from '../components/ui/button'
import { availablePlugins, loadPlugins, managePlugin, pluginError, plugins, pluginsLoading } from '../plugins'

const busy = ref<string>()
const expanded = ref<string>()
const actionError = ref<string>()

async function change(id: string, action: Parameters<typeof managePlugin>[1]) {
  busy.value = id
  actionError.value = undefined
  try {
    await managePlugin(id, action)
    if (action === 'disable' && expanded.value === id) expanded.value = undefined
  } catch (error) {
    actionError.value = error instanceof Error ? error.message : String(error)
  } finally {
    busy.value = undefined
  }
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
              <p v-if="plugin.description" class="mt-1 text-sm text-ink2">{{ plugin.description }}</p>
            </div>
            <AskChip v-if="plugin.enabled && plugin.functions.length" label="Ask Assistant" :words="`Dùng plugin ${plugin.label} để `" />
            <RouterLink v-if="plugin.enabled && plugin.has_settings" :to="`/connections/plugins/${plugin.id}/settings`" class="text-sm font-medium hover:underline">Settings</RouterLink>
            <Button v-if="plugin.enabled"
              variant="ghost" size="sm" :aria-expanded="expanded === plugin.id" :aria-controls="`plugin-details-${plugin.id}`"
              @click="expanded = expanded === plugin.id ? undefined : plugin.id">
              Details <ChevronDown class="size-4 transition-transform" :class="expanded === plugin.id && 'rotate-180'" />
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
          <PluginDetails v-if="expanded === plugin.id && plugin.enabled" :id="`plugin-details-${plugin.id}`" :plugin="plugin" />
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
            <p v-if="plugin.manifest?.description" class="mt-1 text-sm text-ink2">{{ plugin.manifest?.description }}</p>
          </div>
          <details v-if="plugin.manifest" class="text-sm">
            <summary class="cursor-pointer">Package access</summary>
            <p class="mt-2">Version {{ plugin.manifest.version }}</p>
            <p v-for="tool in plugin.manifest.tools" :key="tool.name">{{ tool.name }} · {{ tool.effect }}</p>
            <p v-for="v in plugin.manifest.vars.filter(v => v.vault)" :key="v.name">Vault · {{ v.name }}</p>
            <p v-for="a in plugin.manifest.agents" :key="a.id">Crew · {{ a.name }}</p>
            <p v-for="r in plugin.manifest.automations" :key="r.id">Routine · {{ r.title }}</p>
            <p v-if="plugin.manifest.storage">Storage · {{ plugin.manifest.storage }}</p>
          </details>
          <Button size="sm" :disabled="!!busy" @click="change(plugin.id, 'install')">
            <LoaderCircle v-if="busy === plugin.id" class="size-4 animate-spin" />
            <Download v-else class="size-4" /> Install
          </Button>
        </li>
      </ul>
    </section>
  </div>
</template>
