<script setup lang="ts">
// Which agents may use a Vault item: every agent (those added later too), or the ones ticked. The librarian is never
// one: it reads the owner's words, not his accounts.
import { computed } from 'vue'
import { useRead } from '../../data'

const model = defineModel<{ all_agents: boolean; agents: string[] }>({ required: true })
const { data: crew } = useRead('list_crew')
const agents = computed(() => [
  { id: 'assistant', name: 'Assistant', role: 'Your right hand' },
  ...(crew.value ?? []).filter((m) => m.status === 'active').map((m) => ({ id: m.id, name: m.name, role: m.role })),
])
function toggle(id: string, on: boolean) {
  const set = new Set(model.value.agents)
  if (on) set.add(id)
  else set.delete(id)
  model.value = { all_agents: false, agents: agents.value.map((a) => a.id).filter((a) => set.has(a)) }
}
</script>

<template>
  <div class="space-y-2">
    <div role="radiogroup" aria-label="Who may use it" class="inline-flex h-8 items-center gap-0.5 rounded-lg bg-panel2 p-0.5">
      <button type="button" role="radio" :aria-checked="model.all_agents"
        class="h-7 rounded-md px-2.5 text-[13px] font-medium text-ink2 hover:text-ink aria-checked:bg-panel aria-checked:text-ink aria-checked:shadow-[0_0_0_1px_var(--line2)]"
        @click="model = { all_agents: true, agents: [] }">Every agent</button>
      <button type="button" role="radio" :aria-checked="!model.all_agents"
        class="h-7 rounded-md px-2.5 text-[13px] font-medium text-ink2 hover:text-ink aria-checked:bg-panel aria-checked:text-ink aria-checked:shadow-[0_0_0_1px_var(--line2)]"
        @click="model = { all_agents: false, agents: model.agents }">Only these</button>
    </div>
    <p v-if="model.all_agents" class="text-[13px] text-ink2">The Assistant and every crew member, those you add later too.</p>
    <ul v-else class="overflow-hidden rounded-xl border border-line">
      <li v-for="a in agents" :key="a.id" class="border-b border-line last:border-b-0">
        <label class="flex cursor-pointer items-center gap-3 px-3.5 py-2.5 hover:bg-panel2/60">
          <input type="checkbox" class="size-4 accent-[var(--ink)]" :checked="model.agents.includes(a.id)"
            @change="toggle(a.id, ($event.target as HTMLInputElement).checked)" />
          <span class="min-w-0 flex-1">
            <span class="block truncate text-[14px] font-medium">{{ a.name }}</span>
            <span class="block truncate text-[12.5px] text-ink3">{{ a.role }}</span>
          </span>
        </label>
      </li>
    </ul>
    <p v-if="!model.all_agents && !model.agents.length" class="text-[13px] text-warning">No agent may use it yet.</p>
  </div>
</template>
