<script setup lang="ts">
import { DropdownMenuContent, DropdownMenuItem, DropdownMenuLabel, DropdownMenuPortal, DropdownMenuRoot, DropdownMenuSeparator, DropdownMenuTrigger } from 'reka-ui';
import { ASSISTANT, agentState, talkTo } from '../../assistant';
import AgentBadge from './AgentBadge.vue';
import Icon from './Icon.vue';
import LiveDot from './LiveDot.vue';

// Who the panel talks to, in the drawer header's title place (lifeos's: remit's Assistant is alone in its panel): the
// Assistant and each active crew member, a dot on whoever is at work. Picking one shows its drawer; the crew is managed on
// the Crew page.
const props = defineProps<{ current: { id: string; name: string }; crew: { id: string; name: string; role: string }[] }>();
const emit = defineEmits<{ manage: [] }>();
const everyone = () => [{ id: ASSISTANT, name: 'Assistant', role: 'Your assistant' }, ...props.crew];
const working = (id: string) => agentState(id).view?.card.status === 'working';
</script>

<template>
  <DropdownMenuRoot>
    <DropdownMenuTrigger class="flex max-w-[12rem] min-w-0 flex-none items-center gap-2 rounded-md py-0.5 pr-1 hover:bg-muted" :aria-label="`Talking to ${current.name}. Pick another agent`" data-agent-picker>
      <AgentBadge :id="current.id" :name="current.name" :size="22" />
      <h2 class="min-w-0 truncate text-[14px] font-bold">{{ current.name }}</h2>
      <Icon name="chevron-down" :size="12" class="flex-none text-mut" />
    </DropdownMenuTrigger>
    <DropdownMenuPortal>
      <DropdownMenuContent align="start" :side-offset="6" class="remit z-50 w-64 rounded-lg bg-popover p-1 text-popover-foreground shadow-md ring-1 ring-foreground/10">
        <DropdownMenuLabel class="px-1.5 py-1 text-xs font-medium text-muted-foreground">Talk to</DropdownMenuLabel>
        <DropdownMenuItem v-for="a in everyone()" :key="a.id" class="relative flex cursor-default items-center gap-2 rounded-md px-1.5 py-1 outline-hidden select-none focus:bg-accent" :class="a.id === current.id ? 'bg-accent' : ''" @select="talkTo(a.id)">
          <AgentBadge :id="a.id" :name="a.name" :size="20" />
          <span class="min-w-0 flex-1">
            <span class="block truncate text-[13px] font-medium">{{ a.name }}</span>
            <span class="block truncate text-[11px] text-mut">{{ a.role }}</span>
          </span>
          <span v-if="working(a.id)" class="text-info"><LiveDot live /><span class="sr-only">Working</span></span>
        </DropdownMenuItem>
        <DropdownMenuSeparator class="-mx-1 my-1 h-px bg-border" />
        <DropdownMenuItem class="flex cursor-default items-center gap-2 rounded-md px-1.5 py-1 text-xs text-mut outline-hidden select-none focus:bg-accent" @select="emit('manage')"><Icon name="crew" :size="13" />Manage crew…</DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenuPortal>
  </DropdownMenuRoot>
</template>
