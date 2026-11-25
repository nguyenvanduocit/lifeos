<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useRoute } from 'vue-router'
import PageHeader from '../components/ds/PageHeader.vue'
import IntegrationTabs from '../components/IntegrationTabs.vue'
import { Button } from '../components/ui/button'
import { useVault } from '../vault'
import { plugins } from '../plugins'

interface Field { key: string; label: string; type: 'text'|'secret'|'select'; options: string[]; default: string; required: boolean }
interface View { vault_keys: string[]; schema: { description: string; fields: Field[] }; values: Record<string,string>; saved: string[]; can_reset: boolean }

const route = useRoute()
const id = computed(() => String(route.params.plugin))
const plugin = computed(() => plugins.value.find(p => p.id === id.value))
const endpoint = computed(() => `/api/plugins/${encodeURIComponent(id.value)}/settings`)
const headers = { 'X-LifeOS-Account': '1', 'Content-Type': 'application/json' }
const view = ref<View>()
const values = ref<Record<string,string>>({})
const { data: vault } = useVault()
function vaultTarget(key: string) {
  const item = vault.value?.items.find(i => i.key_name === key)
  return item ? `/connections/vault/${item.id}` : {path:'/connections/vault/new',query:{kind:'key',name:key}}
}
const configFields = computed(() => view.value?.schema.fields.filter(f => !view.value!.vault_keys.includes(f.key)) ?? [])
const busy = ref(false)
const resetOpen = ref(false)
const message = ref('')
async function load(url: string) {
  const response = await fetch(url, { headers, cache: 'no-store' })
  if (!response.ok) throw new Error(await response.text())
  const next: View = await response.json()
  if (url !== endpoint.value) return
  values.value = Object.fromEntries(next.schema.fields.map(f => [f.key, next.values[f.key] ?? f.default ?? '']))
  view.value = next
}
watch(endpoint, async url => {
  view.value = undefined
  values.value = {}
  message.value = ''
  busy.value = false
  resetOpen.value = false
  try { await load(url) } catch (error) { if (endpoint.value === url) message.value = String(error) }
}, { immediate: true })
async function submit(action: 'save'|'reset') {
  if (!view.value) return
  const url = endpoint.value
  busy.value = true
  message.value = ''
  try {
    const fields = configFields.value.filter(f => f.type !== 'secret' || values.value[f.key])
    const changes = Object.fromEntries(fields.map(f => [f.key, values.value[f.key]]))
    const response = await fetch(url, { method: 'POST', headers, body: JSON.stringify(action === 'save' ? {action, values: changes} : {action}) })
    if (!response.ok) throw new Error(await response.text())
    await load(url)
    if (url === endpoint.value) { resetOpen.value = false; message.value = action === 'save' ? 'Settings saved.' : 'Settings cleared.' }
  } catch (error) { if (url === endpoint.value) message.value = String(error) }
  finally { if (url === endpoint.value) busy.value = false }
}
</script>
<template>
  <PageHeader :title="`${plugin?.label || id} settings`" />
  <IntegrationTabs />
  <section class="max-w-lg space-y-4 rounded-2xl border border-line bg-panel p-5">
    <template v-if="view">
      <p v-if="view.schema.description" class="text-sm text-ink2">{{ view.schema.description }}</p>
      <div v-for="key in view.vault_keys" :key="key" class="flex flex-wrap items-center justify-between gap-2 text-sm">
        <code>{{ key }}</code><RouterLink class="underline" :to="vaultTarget(key)">{{ view.saved.includes(key) ? 'Manage in Vault' : 'Open Vault' }}</RouterLink>
      </div>
      <form v-if="configFields.length" class="space-y-4" @submit.prevent="submit('save')">
        <label v-for="f in configFields" :key="f.key" class="block">{{ f.label }}
          <select v-if="f.type === 'select'" v-model="values[f.key]" :required="f.required" class="mt-1 block w-full rounded-md border border-line bg-panel p-2">
            <option v-if="!f.required" value=""></option><option v-for="o in f.options" :key="o" :value="o">{{ o }}</option>
          </select>
          <input v-else v-model="values[f.key]" :type="f.type === 'secret' ? 'password' : 'text'" :autocomplete="f.type === 'secret' ? 'new-password' : 'off'"
            :required="f.required && !(f.type === 'secret' && view.saved.includes(f.key))"
            :placeholder="f.type === 'secret' && view.saved.includes(f.key) ? 'Saved · leave blank to keep' : ''" class="mt-1 block w-full rounded-md border border-line p-2" />
        </label>
        <Button type="submit" :disabled="busy">Save settings</Button>
      </form>
      <div v-if="configFields.some(f => view?.saved.includes(f.key)) && view.can_reset">
        <Button v-if="!resetOpen" variant="outline" @click="resetOpen = true">Clear settings</Button>
        <template v-else>
          <p class="text-sm">Remove all saved settings for {{ plugin?.label || id }}?</p>
          <Button :disabled="busy" @click="submit('reset')">Clear saved settings</Button>
          <Button variant="ghost" @click="resetOpen = false">Cancel</Button>
        </template>
      </div>
      <RouterLink v-for="slot in plugin?.slots.filter(s => s.slot === 'page')" :key="slot.id" class="block text-sm underline" :to="`/plugins/${id}/${slot.id}`">{{ slot.title }}</RouterLink>
    </template>
    <p role="status" class="text-sm">{{ message }}</p>
  </section>
</template>
