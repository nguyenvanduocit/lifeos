<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useEventListener } from '@vueuse/core'
import { callPlugin, type Plugin, type PluginSlot } from '../plugins'
import type { Collection, Field, Item } from '../pluginCollection'
import Panel from './ds/Panel.vue'
import { Button } from './ui/button'

const props = defineProps<{ schema: Collection; plugin: Plugin; slot: PluginSlot }>()
const s = props.schema
const widget = computed(() => props.slot.slot === 'dashboard')
function today() {
  const now = new Date()
  return new Date(now.getTime() - now.getTimezoneOffset() * 60000).toISOString().slice(0, 10)
}
const month = ref(today().slice(0, 7))
const rows = ref<Item[]>([])
const editing = ref<Item>()
const deleting = ref<Item>()
const draft = ref<Item>({})
const busy = ref(false)
const loading = ref(true)
const error = ref('')
const actionError = ref('')
let revision = 0

async function refresh() {
  const current = ++revision
  loading.value = true
  error.value = ''
  try {
    if (!month.value) throw new Error(s.period.label)
    const result = JSON.parse(await callPlugin(props.plugin, s.read, { [s.period.parameter]: month.value }))
    if (!Array.isArray(result) || !result.every(row => row && ['string','number'].includes(typeof row[s.key]) && Number.isFinite(row[s.summary.amount]))) throw new Error('Invalid collection rows')
    if (revision === current) rows.value = result
  } catch (e) {
    if (revision === current) { rows.value = []; error.value = String(e) }
  } finally {
    if (revision === current) loading.value = false
  }
}
watch(month, refresh, { immediate: true })
useEventListener(document, 'visibilitychange', () => {
  if (!document.hidden) {
    if (widget.value && month.value !== today().slice(0,7)) month.value = today().slice(0,7)
    else void refresh()
  }
})

const money = new Intl.NumberFormat(s.summary.locale, { style: 'currency', currency: s.summary.currency })
const groups = s.fields.find(f => f.key === s.summary.group)!.options!
const details = s.fields.filter(f => f.key !== s.summary.group && f.key !== s.summary.amount)
const summary = computed(() => {
  const amounts = new Map<string, number>()
  let total = 0
  for (const row of rows.value) {
    const group = String(row[s.summary.group])
    const value = Number(row[s.summary.amount])
    total += value
    amounts.set(group, (amounts.get(group) ?? 0) + value)
  }
  return { total, amounts }
})
function describe(row: Item) {
  return details.filter(f => row[f.key] != null && row[f.key] !== '').map(f => {
    const value = String(row[f.key])
    if (f.type === 'date') return new Date(value+'T12:00:00').toLocaleDateString(s.summary.locale)
    return [f.options?.[value] ?? value, f.unit].filter(Boolean).join(' ')
  }).join(' · ')
}
function start(row?: Item) {
  editing.value = row
  deleting.value = undefined
  actionError.value = ''
  draft.value = Object.fromEntries(s.fields.map(f => [f.key, row?.[f.key] ??
    (f.type === 'date' ? today() : f.type === 'select' ? Object.keys(f.options!)[0] : '')]))
}
start()
const visible = (f: Field) => !f.when || draft.value[f.when.key] === f.when.equals
async function mutate(remove?: Item) {
  if (busy.value) return
  busy.value = true
  actionError.value = ''
  try {
    if (remove) {
      await callPlugin(props.plugin, s.remove, { [s.key]: remove[s.key] })
      deleting.value = undefined
      if (editing.value?.[s.key] === remove[s.key]) start()
    } else {
      const payload: Item = { [s.key]: editing.value?.[s.key] ?? null }
      for (const f of s.fields) {
        const value = draft.value[f.key]
        payload[f.key] = !visible(f) ? null : f.type !== 'number' ? value : value === '' ? null : Number(value)
      }
      await callPlugin(props.plugin, s.save, payload)
      const selected = String(payload[s.period.field]).slice(0, 7)
      start()
      if (selected !== month.value) { month.value = selected; return }
    }
    await refresh()
  } catch (e) { actionError.value = String(e) }
  finally { busy.value = false }
}
const control = 'min-w-0 rounded-lg border border-line2 bg-panel px-3 py-2 text-sm text-ink'
</script>
<template>
  <section class="space-y-5" :aria-label="slot.title" data-plugin-ui="collection">
    <Panel :title="slot.title" pad>
      <template v-if="!widget" #actions>
        <label class="flex items-center gap-2 text-sm text-ink2">{{ s.period.label }}
          <input v-model="month" type="month" required :class="control" :disabled="busy" />
        </label>
      </template>
      <p v-if="error" role="alert" class="text-critical">{{ error }}</p>
      <div v-else aria-live="polite">
        <p class="text-sm text-ink2">{{ s.summary.label }} · {{ month.split('-').reverse().join('/') }}</p>
        <p class="text-3xl font-semibold tabular-nums">{{ loading ? '…' : money.format(summary.total) }}</p>
        <div v-if="!loading" class="mt-2 flex flex-wrap gap-x-4 gap-y-1 text-sm text-ink2">
          <span>{{ rows.length }} {{ s.summary.count }}</span>
          <span v-for="[key, amount] in summary.amounts" :key="key">{{ groups[key] }} · {{ money.format(amount) }}</span>
        </div>
      </div>
    </Panel>
    <template v-if="!widget">
      <Panel :title="editing ? s.labels.edit : s.labels.create" pad>
        <form @submit.prevent="mutate()"><fieldset :disabled="busy" class="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
          <label v-for="f in s.fields.filter(visible)" :key="f.key" class="grid gap-1 text-sm font-medium">{{ f.label }}
            <select v-if="f.type === 'select'" v-model="draft[f.key]" :name="f.key" :class="control">
              <option v-for="(label, value) in f.options" :key="value" :value="value">{{ label }}</option>
            </select>
            <input v-else v-model="draft[f.key]" :type="f.type" :name="f.key" :required="f.required" :min="f.min" :max="f.max" :step="f.step" :maxlength="f.maxlength" :class="control" />
          </label>
        </fieldset><div class="mt-4 flex gap-2">
          <Button type="submit" :disabled="busy">{{ s.labels.submit }}</Button>
          <Button v-if="editing" type="button" variant="outline" :disabled="busy" @click="start()">{{ s.labels.cancel }}</Button>
        </div></form>
      </Panel>
      <p v-if="actionError" role="alert" class="text-critical">{{ actionError }}</p>
      <Panel :title="s.labels.history" pad>
        <ul v-if="rows.length && !loading" class="divide-y divide-line">
          <li v-for="row in rows" :key="String(row[s.key])" class="flex flex-wrap items-center gap-3 py-3">
            <div class="min-w-40 flex-1"><strong>{{ groups[String(row[s.summary.group])] }}</strong>
              <p class="break-words text-sm text-ink2">{{ describe(row) }}</p></div>
            <strong class="tabular-nums">{{ money.format(Number(row[s.summary.amount])) }}</strong>
            <template v-if="deleting?.[s.key] === row[s.key]">
              <Button size="sm" :disabled="busy" @click="mutate(row)">{{ s.labels.confirm }}</Button>
              <Button size="sm" variant="outline" :disabled="busy" @click="deleting = undefined">{{ s.labels.cancel }}</Button>
            </template>
            <template v-else>
              <Button size="sm" variant="outline" :disabled="busy" @click="start(row)">{{ s.labels.modify }}</Button>
              <Button size="sm" variant="ghost" :disabled="busy" @click="deleting = row">{{ s.labels.delete }}</Button>
            </template>
          </li>
        </ul>
        <p v-else-if="!error && !loading" class="text-sm text-ink2">{{ s.labels.empty }}</p>
      </Panel>
    </template>
  </section>
</template>
