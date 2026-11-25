import { computed, ref } from 'vue'

export interface PluginSlot {
  id: string
  slot: 'page' | 'dashboard' | 'sidebar'
  title: string
  expose: string
}

export interface PluginPackage {
  agents?: { id: string; name: string; description?: string }[]
  automations?: { id: string; title: string; cron?: string; on?: string[]; paused?: boolean }[]
  manifest?: { agents:{id:string;name:string}[]; automations?:{id:string;title:string}[]; description: string; version: string; storage?:string; vars: {name:string; vault?:boolean}[]; tools:{name:string; effect:string}[] }
  description?: string
  id: string
  label: string
}

export interface Plugin extends PluginPackage {
  vault_keys?: string[]
  emits?: Record<string, string>
  has_settings?: boolean
  capability?: string
  processors?: string[]
  retrying_deliveries: number
  configured?: boolean
  chats?: number[]
  enabled: boolean
  functions: { name: string; description: string; effect?: string }[]
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

export const sidebarPages = computed(() => plugins.value.filter(p => p.enabled).flatMap(plugin =>
  plugin.slots.filter(s => s.slot === 'sidebar').map(slot => ({
    plugin, slot, path: `/plugins/${encodeURIComponent(plugin.id)}/${encodeURIComponent(slot.id)}`,
    name: slot.title || plugin.label, domain: 'integrations' as const,
  }))
))
