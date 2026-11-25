<script setup lang="ts">
// A detail's facts as label | value rows: the label in grey, 112px wide; `value` decides whether the row shows (empty:
// left out), a named slot may draw it richer. `stack` puts the label above the value (narrow side columns).
import type { Component } from 'vue'

defineProps<{ rows: { key: string; label: string; icon?: Component; value?: string | number | null }[]; stack?: boolean }>()
</script>

<template>
  <dl class="grid gap-x-4 gap-y-0 text-[13.5px]" :class="stack ? 'grid-cols-1' : 'grid-cols-[7rem_minmax(0,1fr)]'">
    <template v-for="r in rows" :key="r.key">
      <template v-if="r.value !== undefined && r.value !== null && r.value !== ''">
        <dt class="flex items-center gap-2 text-ink3" :class="stack ? 'mt-3 h-6 first:mt-0' : 'h-9'">
          <component :is="r.icon" v-if="r.icon" class="size-3.5" aria-hidden="true" />{{ r.label }}
        </dt>
        <dd class="flex min-w-0 flex-wrap items-center gap-2 text-ink" :class="stack ? 'min-h-7' : 'min-h-9'">
          <slot :name="r.key">{{ r.value }}</slot>
        </dd>
      </template>
    </template>
  </dl>
</template>
