import { ref } from 'vue'

export interface PluginSlot {
  id: string
  slot: 'page' | 'dashboard'
  title: string
  expose: string
}

export interface PluginPackage {
  id: string
  label: string
}

export interface Plugin extends PluginPackage {
  retrying_deliveries: number
  enabled: boolean
  functions: { name: string; description: string }[]
  slots: PluginSlot[]
  error?: string
}

export const plugins = ref<Plugin[]>([])
export const availablePlugins = ref<PluginPackage[]>([])
export const pluginError = ref<string | null>(null)
export const pluginsLoading = ref(true)

let refreshPending = false
let loading: Promise<void> | undefined
export function loadPlugins() {
  if (loading) {
    refreshPending = true
    return loading
  }
  return loading = (async () => {
    pluginsLoading.value = true
    try {
      do {
        refreshPending = false
        const response = await fetch('/api/plugins', { cache: 'no-store' })
        if (!response.ok) throw new Error('Could not load plugins')
        const catalog = await response.json()
        plugins.value = catalog.plugins
        availablePlugins.value = catalog.available
        pluginError.value = catalog.error
      } while (refreshPending)
    } catch (error) {
      pluginError.value = error instanceof Error ? error.message : String(error)
    } finally {
      pluginsLoading.value = false
      loading = undefined
    }
  })()
}

export async function callPlugin(plugin: Plugin, name: string, args: unknown): Promise<string> {
  if (!plugin.functions.some((tool) => tool.name === name)) throw new Error('Unknown plugin function')
  const response = await fetch(`/api/plugins/${encodeURIComponent(plugin.id)}/functions/${encodeURIComponent(name)}`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ arguments: args }),
  })
  if (!response.ok) throw new Error(await response.text())
  return (await response.json()).result
}

export async function managePlugin(id: string, action: 'install' | 'enable' | 'disable') {
  const response = await fetch(`/api/plugins/${encodeURIComponent(id)}`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json', 'X-LifeOS-Plugin': '1' },
    body: JSON.stringify(action),
  })
  if (!response.ok) throw new Error(await response.text())
  await loadPlugins()
}
