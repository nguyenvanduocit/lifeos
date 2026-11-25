<script setup lang="ts">
import Tip from './Tip.vue';
import { Tabs, TabsList } from './ui/tabs';

// The header of a page or a drawer about one thing (a chain, an event, an agent, an event signal): its first row says what it
// is (`title`, in code type when it is a name the owner types: `mono`; a drawer that draws its own, an agent's badge and name,
// fills the `title` slot) and where it stands (`standing`), with what the owner can do to it at the end (`end`); a page with
// tabs has a second row with them (`tabs`, the one open is `tab`) and, at its end, what the page shows of it at a glance
// (`aside`). The content starts right under it. Both rows keep one height, so the headers of columns side by side line up row
// for row whatever they hold: what does not fit truncates inside the header's width (it is a container), it never wraps.
defineProps<{ title: string; mono?: boolean }>();
const tab = defineModel<string>('tab', { default: '' });
const Heading = 'h2';
</script>

<template>
  <div class="head @container flex flex-col border-b border-line bg-bg2 px-3.5">
    <div class="flex h-11 min-w-0 items-center gap-x-3">
      <slot name="title"><Tip :text="title" when-clipped><component :is="Heading" class="min-w-0 truncate text-sm font-[650]" :class="{ 'font-mono': mono }">{{ title }}</component></Tip></slot>
      <slot name="standing" />
      <div class="ml-auto flex flex-none items-center justify-end gap-2"><slot name="end" /></div>
    </div>
    <div v-if="$slots.tabs" class="flex min-w-0 items-center gap-3">
      <!-- The active tab's underline hangs onto the header's border, below the tabs: the padding keeps it inside this scroll
           box (or the box scrolls vertically), the negative margin keeps the row's height. -->
      <Tabs class="-mb-0.5 max-w-full min-w-0 overflow-x-auto pb-0.5" :model-value="tab" @update:model-value="tab = String($event)">
        <TabsList variant="line" class="h-9 gap-4"><slot name="tabs" /></TabsList>
      </Tabs>
      <div class="flex min-w-0 flex-1 justify-end gap-2"><slot name="aside" /></div>
    </div>
  </div>
</template>
