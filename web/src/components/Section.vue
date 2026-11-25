<script setup lang="ts">
import type { Component } from 'vue'
import { ChevronRight } from '@lucide/vue'
import { Badge } from './ui/badge'
import { Button } from './ui/button'
import { Card, CardContent, CardHeader } from './ui/card'

defineProps<{ title: string; to?: string; count?: number; icon?: Component }>()
</script>

<template>
  <Card class="min-w-0 gap-3 rounded-2xl border-line py-5 shadow-none">
    <CardHeader class="flex flex-wrap items-center gap-2 px-5">
      <span v-if="icon" class="grid size-9 shrink-0 place-items-center rounded-xl bg-primary/10 text-primary">
        <component :is="icon" class="size-4" />
      </span>
      <h2 class="text-base font-semibold tracking-tight">{{ title }}</h2>
      <Badge v-if="count" variant="secondary" class="tabular-nums">{{ count }}</Badge>
      <div v-if="$slots.actions || to" class="ml-auto flex flex-wrap items-center gap-1">
        <slot name="actions" />
        <Button v-if="to" as-child variant="ghost" size="xs" class="text-ink2">
          <RouterLink :to="to" :aria-label="'Xem ' + title">
            Xem <ChevronRight />
          </RouterLink>
        </Button>
      </div>
    </CardHeader>
    <CardContent class="px-5">
      <slot />
    </CardContent>
  </Card>
</template>
