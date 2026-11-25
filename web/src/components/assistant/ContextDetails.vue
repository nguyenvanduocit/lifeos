<script setup lang="ts">
import { computed, onBeforeUnmount, useId, useTemplateRef, watchPostEffect } from 'vue';
import * as link from '../../core/link';
import { useRemote } from './useRemote';
import { allowedText, mcpLook, tokens } from './look';
import type { Card } from '../../core/generated/Card';
import Ago from './Ago.vue';
import CodeBlock from './CodeBlock.vue';
import ContextFold from './ContextFold.vue';
import ContextList from './ContextList.vue';
import EmptyState from './EmptyState.vue';
import ListState from './ListState.vue';
import KeyValues from './KeyValues.vue';
import StandingBadge from './StandingBadge.vue';
import Tip from './Tip.vue';
import { Badge } from './ui/badge';

// What fills an agent's context, in its Session panel, for the owner to understand it: Claude Code's own count of each
// part (what `/context` shows), what LifeOS gave it and what Claude Code loaded. Read only: the owner changes an agent by
// talking to it. `reveal` asks to scroll here (the context meter does), done once there is something to show, so the
// section lands at the top of the panel.
const props = defineProps<{ card: Card }>();
const reveal = defineModel<boolean>('reveal', { default: false });

// Asked while the panel shows, and again as each turn starts and ends: Claude Code says what the session loaded just
// after a turn starts, and counts its context as it rests.
const remote = useRemote(
  () => ({ cardId: props.card.id, turns: props.card.turns, status: props.card.status }),
  ({ cardId }) => link.context(cardId),
);
const ctx = computed(() => (remote.of.value?.cardId === props.card.id ? remote.data.value : undefined));
const counted = computed(() => ctx.value?.context?.context ?? null);
const countedAt = computed(() => ctx.value?.context?.at ?? '');
const given = computed(() => ctx.value?.given ?? null);
const share = (n: number) => (counted.value && counted.value.size > 0 ? Math.min(100, (n / counted.value.size) * 100) : 0);
// The parts but the space left, which the bar's empty rest already shows.
const parts = computed(() => (counted.value?.parts ?? []).filter((p) => p.kind !== 'free'));
const sent = computed(() => parts.value.filter((p) => p.kind === 'used'));
const largest = computed(() => Math.max(1, ...parts.value.map((p) => p.tokens)));
const KIND_NOTE: Record<string, string> = { deferred: 'listed by name until loaded', buffer: 'kept free to compact' };
const lists = computed(() =>
  counted.value
    ? [
        { title: 'Agents', items: counted.value.agents },
        { title: 'Tools used in the conversation', items: counted.value.tool_calls },
      ]
    : [],
);
const byTokens = <T extends { tokens: number }>(xs: readonly T[]) => [...xs].sort((a, b) => b.tokens - a.tokens);
const byName = (x: { name: string }) => x.name;
const same = (x: string) => x;
const ids = { tokens: useId(), given: useId(), loaded: useId() };

const root = useTemplateRef<HTMLElement>('root');
watchPostEffect(() => {
  if (!reveal.value || !root.value || (!ctx.value && !remote.error.value)) return;
  root.value.scrollIntoView({ block: 'start' });
  reveal.value = false;
});
onBeforeUnmount(() => (reveal.value = false)); // a reveal left for a panel the owner moved away from is dropped
</script>

<template>
  <section ref="root" aria-label="What fills the context" class="scroll-mt-4">
    <section :aria-labelledby="ids.tokens">
      <h2 :id="ids.tokens" class="caps mb-1.5">Context</h2>
      <p class="mb-2.5 text-[11.5px] text-mut">What fills this agent's context, as Claude Code counts it (its <span class="mono">/context</span>).</p>
      <ListState what="the context" :held="!!ctx" :error="remote.error.value" :retry="remote.retry">
      <template v-if="ctx">
        <div v-if="ctx.context_error" class="mb-2 text-[12px] text-amber" role="status">Not counted now: {{ ctx.context_error }}<template v-if="counted">. This is the count of <Ago :at="countedAt" />.</template></div>
        <EmptyState v-if="!counted" class="text-[12px]">Claude Code has not counted this context yet: it does when the agent's session runs.</EmptyState>
        <template v-else>
          <div class="flex flex-wrap items-baseline gap-x-5 gap-y-1 text-[12.5px] text-mut">
            <span><b class="text-[15px] text-text">{{ tokens(counted.total) }}</b> of {{ tokens(counted.size) }} tokens</span>
            <span>counted <Ago :at="countedAt" /></span>
          </div>
          <div class="stack mt-2" role="img" :aria-label="`${counted.total} of ${counted.size} tokens used`">
            <i v-for="p in sent" :key="p.id" :class="`p-${p.id}`" :style="{ width: share(p.tokens) + '%' }"></i>
          </div>
          <table class="parts mt-2.5 w-full">
            <tbody>
              <tr v-for="p in parts" :key="p.id" :class="{ 'text-faint': p.kind !== 'used' }">
                <td class="w-[210px]"><span class="swatch" :class="p.kind === 'used' ? `p-${p.id}` : 'p-off'"></span>{{ p.name }}<span v-if="KIND_NOTE[p.kind]" class="block pl-[15px] text-[11px] text-faint">{{ KIND_NOTE[p.kind] }}</span></td>
                <td class="w-[80px] text-right tabular-nums">{{ tokens(p.tokens) }}</td>
                <td class="bar-cell"><span class="bar" :class="p.kind === 'used' ? `p-${p.id}` : 'p-off'" :style="{ width: (p.tokens / largest) * 100 + '%' }"></span></td>
              </tr>
            </tbody>
          </table>
          <div class="mt-2">
            <ContextList title="Memory files" :items="byTokens(counted.memory_files)" :text="(m) => m.name" open empty="None: no CLAUDE.md reached this session">
              <template #default="{ item }">
                <div class="flex items-baseline gap-2">
                  <Badge variant="quiet" class="w-[58px]">{{ item.source }}</Badge><Tip :text="item.name" when-clipped><span class="path mono min-w-0 flex-1"><bdi>{{ item.name }}</bdi></span></Tip><span class="text-mut tabular-nums">{{ tokens(item.tokens) }}</span>
                </div>
              </template>
            </ContextList>
            <ContextList v-for="l in lists" :key="l.title" :title="l.title" :items="byTokens(l.items)" :text="(i) => i.name">
              <template #default="{ item }">
                <div class="flex items-baseline gap-2">
                  <span class="mono min-w-0 flex-1 truncate text-text">{{ item.name }}</span>
                  <span v-if="item.deferred" class="text-faint">deferred</span>
                  <span v-if="item.source" class="text-faint">{{ item.source }}</span>
                  <span class="w-[48px] text-right text-mut tabular-nums">{{ tokens(item.tokens) }}</span>
                </div>
              </template>
            </ContextList>
          </div>
        </template>
      </template>
      </ListState>
    </section>

    <template v-if="ctx">
      <section :aria-labelledby="ids.given" class="mt-5">
        <h2 :id="ids.given" class="caps mb-1.5">LifeOS gave it</h2>
        <p class="mb-2.5 text-[11.5px] text-mut">What LifeOS started this session with, recorded as it started it.</p>
        <EmptyState v-if="!given" class="text-[12px]">No data yet: LifeOS records it when it next starts this agent's session.</EmptyState>
        <template v-else>
          <KeyValues>
            <dt>Skills</dt><dd>{{ given.skills.join(', ') || 'none' }}</dd>
            <dt>Claude Code tools</dt><dd>{{ allowedText(given.claude_tools) }}</dd>
            <dt>Settings layers</dt><dd>{{ given.setting_sources.join(', ') || 'none' }}</dd>
            <dt>MCP servers</dt><dd>{{ given.mcp_servers.join(', ') || 'none' }}</dd>
          </KeyValues>
          <div class="mt-2">
            <ContextList title="LifeOS tools" :items="given.tools" :text="byName">
              <template #default="{ item }"><b class="mono text-text">{{ item.name }}</b> <span class="text-mut">{{ item.description }}</span></template>
            </ContextList>
            <ContextList title="Allowed without asking" :items="given.allowed_tools" :text="same" open />
            <ContextList title="Deny rules" :items="given.deny_rules" :text="same">
              <template #default="{ item }"><span class="mono">{{ item }}</span></template>
            </ContextList>
            <ContextFold title="Opening message">
              <CodeBlock v-if="given.opening != null" class="max-h-[260px] overflow-auto" :text="given.opening" label="Copy the opening message" wrap />
              <div v-else class="text-[12px] text-faint">No data: LifeOS records the first message of each session it opens, as it sends it.</div>
            </ContextFold>
          </div>
        </template>
      </section>

      <section :aria-labelledby="ids.loaded" class="mt-5">
        <h2 :id="ids.loaded" class="caps mb-1.5">Claude Code loaded</h2>
        <div v-if="!ctx.loaded" class="mb-2 text-[12px] text-mut">Claude Code has not said yet what this session loaded: it does at the start of the agent's next turn.</div>
        <KeyValues v-else>
          <dt>Claude Code</dt><dd>{{ ctx.loaded.init.version || 'unknown version' }}</dd>
          <dt>Runs</dt><dd>{{ ctx.loaded.init.model }} · mode {{ ctx.loaded.init.permission_mode }}</dd>
          <dt>Output style</dt><dd>{{ ctx.loaded.init.output_style }}</dd>
          <dt>Reported</dt><dd><Ago :at="ctx.loaded.at" /></dd>
        </KeyValues>
        <div class="mt-2">
          <template v-if="ctx.loaded">
            <ContextList title="MCP servers" :items="ctx.loaded.init.mcp_servers" :text="byName" open>
              <template #default="{ item }">
                <div class="flex items-center gap-2"><StandingBadge :variant="mcpLook(item.status).tone" :icon="mcpLook(item.status).icon">{{ mcpLook(item.status).label }}</StandingBadge><span class="text-text">{{ item.name }}</span><span v-if="item.source" class="text-faint">{{ item.source }}</span></div>
              </template>
            </ContextList>
            <ContextList title="Plugins" :items="ctx.loaded.init.plugins" :text="byName">
              <template #default="{ item }"><span class="text-text">{{ item.name }}</span> <span v-if="item.version" class="text-faint">{{ item.version }}</span></template>
              <template v-if="ctx.loaded.init.plugin_errors.length" #note><span class="text-red-text">{{ ctx.loaded.init.plugin_errors.length }} failed to load</span></template>
            </ContextList>
            <ContextList v-if="ctx.loaded.init.plugin_errors.length" title="Plugin errors" :items="ctx.loaded.init.plugin_errors" :text="(e) => e.plugin" open>
              <template #default="{ item }"><span class="text-red-text">{{ item.plugin }}</span> <span class="text-faint">{{ item.kind }}</span> <span class="text-mut">{{ item.message }}</span></template>
            </ContextList>
            <ContextList title="Tools" :items="ctx.loaded.init.tools" :text="same" grid />
            <ContextList title="Slash commands" :items="ctx.loaded.init.slash_commands" :text="same" grid />
          </template>
        </div>
      </section>
    </template>
  </section>
</template>

<style scoped>
/* The parts' colours, by Claude Code's part ids: the same in the stacked bar, the swatches and the rows' bars. A part
   not sent with every request is drawn off. */
.p-system_prompt { background: var(--violet); }
.p-system_tools { background: var(--blue); }
.p-mcp_tools,
.p-mcp_instructions { background: var(--cyan); }
.p-memory_files { background: var(--amber); }
.p-skills { background: var(--green); }
.p-custom_agents { background: var(--pink); }
.p-messages { background: var(--meter-on); }
.p-off { background: var(--line); }
.stack {
  position: relative;
  display: flex;
  height: 10px;
  border-radius: 3px;
  background: var(--panel3);
  overflow: hidden;
}
.parts td {
  padding: 3px 0;
  font-size: 12px;
  color: var(--text2);
  vertical-align: middle;
}
.parts td.bar-cell {
  padding-left: 12px;
}
.swatch {
  display: inline-block;
  width: 8px;
  height: 8px;
  margin-right: 7px;
  border-radius: 2px;
}
/* a long path gives way at its start: the file's own name is what tells it apart */
.path {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  direction: rtl;
  text-align: left;
}
.bar {
  display: block;
  height: 6px;
  min-width: 2px;
  border-radius: 2px;
  opacity: 0.75;
}
</style>
