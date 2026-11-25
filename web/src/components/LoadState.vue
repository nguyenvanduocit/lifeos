<script setup lang="ts">
import { CircleAlert, LoaderCircle } from '@lucide/vue'
import { Button } from './ui/button'

defineProps<{ loading?: boolean; error?: string; label: string }>()
defineEmits<{ retry: [] }>()
</script>

<template>
  <div v-if="error && !loading" role="alert"
    class="flex flex-wrap items-center gap-x-3 gap-y-2 rounded-xl border border-critical/25 bg-critical-wash px-4 py-3 text-sm text-critical"
    :title="error">
    <CircleAlert class="size-4 shrink-0" aria-hidden="true" />
    <span class="min-w-0 flex-1">Could not load {{ label }}.</span>
    <Button type="button" size="sm" variant="outline" @click="$emit('retry')">Try again</Button>
  </div>
  <div v-else-if="loading" role="status" class="flex items-center gap-2 py-3 text-sm text-ink2">
    <LoaderCircle class="size-4 shrink-0 motion-safe:animate-spin" aria-hidden="true" />
    Loading {{ label }}…
  </div>
</template>
