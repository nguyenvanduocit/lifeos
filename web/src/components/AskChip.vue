<script setup lang="ts">
// A page action: it puts words in the Assistant's composer for the owner to finish and send.
// Relay's buttons: outlined in ink, or solid black for the page's main action; the icon says what it does and the
// small ✦ on inline actions says the Assistant does it.
import type { Component } from 'vue'
import { Button } from './ui/button'
import { say } from '../assistant'

defineProps<{ words: string; label?: string; icon?: Component; size?: 'xs' | 'sm'; primary?: boolean; ghost?: boolean; iconOnly?: boolean }>()
</script>

<template>
  <Button
    type="button"
    :variant="primary ? 'default' : ghost ? 'ghost' : 'outline'"
    :size="iconOnly && icon ? 'icon-sm' : size ?? 'xs'"
    class="group/ask font-medium"
    :class="[
      size === 'sm' || (iconOnly && icon) ? 'rounded-lg text-[13px]' : 'h-8 rounded-[10px] px-3 text-[13px]',
      primary ? '' : 'border-line2 bg-panel text-ink hover:bg-panel2',
    ]"
    :aria-label="(label ?? words) + ' · drafts it in the Assistant'"
    :title="(label ?? words) + ' · drafts it in the Assistant'"
    @click="say(words)"
  >
    <component :is="icon" v-if="icon" class="size-3.5" aria-hidden="true" />
    <span v-if="!iconOnly || !icon">{{ label ?? words }}</span>
    <svg v-if="(!iconOnly || !icon) && (size !== 'sm' || !icon)" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"
      class="-mr-0.5 size-2.5 transition-transform duration-300 group-hover/ask:rotate-90"
      :class="primary ? 'opacity-60' : 'text-ink3'">
      <path d="M12 1.5c.4 5.6 4.9 10.1 10.5 10.5-5.6.4-10.1 4.9-10.5 10.5C11.6 16.9 7.1 12.4 1.5 12 7.1 11.6 11.6 7.1 12 1.5Z" />
    </svg>
  </Button>
</template>
