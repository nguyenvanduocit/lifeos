<script setup lang="ts" generic="T extends string">
// A few views of the same list, one chosen: the chosen one is a raised tile in a grey well. A count may follow a label.
defineProps<{ options: readonly { value: T; label: string; count?: number; urgent?: boolean }[]; label: string }>()
const model = defineModel<T>({ required: true })
</script>

<template>
  <div role="radiogroup" :aria-label="label" class="inline-flex h-8 max-w-full shrink-0 items-center gap-0.5 overflow-x-auto rounded-lg bg-panel2 p-0.5 [scrollbar-width:none]">
    <button
      v-for="o in options"
      :key="o.value"
      type="button"
      role="radio"
      :aria-checked="model === o.value"
      class="inline-flex h-7 shrink-0 items-center gap-1.5 rounded-md px-2.5 whitespace-nowrap text-[13px] font-medium text-ink2 transition-colors hover:text-ink focus-visible:outline-2 focus-visible:outline-ring aria-checked:bg-panel aria-checked:text-ink aria-checked:shadow-[0_0_0_1px_var(--line2)]"
      @click="model = o.value"
    >
      {{ o.label }}
      <span v-if="o.count !== undefined" class="font-mono text-[11px] tabular-nums" :class="o.urgent && o.count ? 'text-critical' : 'text-ink3'">{{ o.count }}</span>
    </button>
  </div>
</template>
