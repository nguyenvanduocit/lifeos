<script setup lang="ts">
import { computed, nextTick, ref, useTemplateRef, watch } from 'vue';
import { useElementSize } from '@vueuse/core';
import type { Memory } from '../../core/generated/Memory';
import { say } from '../../assistant';
import { useRead } from '../../data';
import { useQueryRef } from '../../nav';
import { short } from './look';
import EmptyState from './EmptyState.vue';
import ListState from './ListState.vue';
import MemoryCard from './MemoryCard.vue';
import SearchBox from './SearchBox.vue';
import TeachingCard from './TeachingCard.vue';
import { Badge } from './ui/badge';
import { Tabs, TabsContent, TabsList, TabsTrigger } from './ui/tabs';

// What one agent's memory holds (`holder`: the Assistant's, a crew member's), as remit's MemoryBook: one tab for what it
// recalls now, one for what it once held and stopped recalling (superseded, forgotten), one for the teachings the librarian
// learned from. The tabs and a search over the words stay at the top of the scroll; which tab shows and what the search
// holds are view state, in the address under `viewKey` (and `<viewKey>Q`). What one card names (a memory it links to or was
// replaced by, the teaching it came from, what a teaching wrote) opens in its own tab, scrolled to, focused and marked as
// where the link went; so does a memory the stream says a turn was given (`land`, cleared once it is shown). A forget is the
// owner's words to the Assistant, put in its box for him to send: only the Assistant forgets.
const props = defineProps<{ holder: string; who: string; viewKey: string }>();
const land = defineModel<number | null>('land', { default: null });

const { data: memoryData, error: memoryError, retry: retryMemories } = useRead('list_memories', () => ({ holder: props.holder, include_forgotten: true }));
const { data: teachingData, error: teachingError, retry: retryTeachings } = useRead('list_teachings', () => ({ holder: props.holder }));
const error = computed(() => memoryError.value || teachingError.value);
const held = computed(() => !!memoryData.value && !!teachingData.value);
const retry = () => {
  retryMemories();
  retryTeachings();
};

const VIEWS = ['now', 'ended', 'teachings'] as const;
type View = (typeof VIEWS)[number];
const view = useQueryRef<View>(props.viewKey, 'now', VIEWS);
const query = useQueryRef<string>(`${props.viewKey}Q`, '');

const memories = computed(() => memoryData.value ?? []);
const teachings = computed(() => teachingData.value ?? []);
const byId = computed(() => new Map(memories.value.map((m) => [m.id, m])));
const find = (id: number) => byId.value.get(id);
const teachingById = computed(() => new Map(teachings.value.map((t) => [t.id, t])));
const taughtIn = (m: Memory) => (m.source == null ? undefined : teachingById.value.get(m.source));
// A memory a later one corrects is replaced by it: the later one names it with its `supersedes` link.
const replacer = computed(() => {
  const out = new Map<number, Memory>();
  for (const m of memories.value) for (const l of m.links) if (l.kind === 'supersedes') out.set(l.to, m);
  return out;
});

// The search keeps the cards whose words hold every word typed, in any case.
const words = computed(() => query.value.toLowerCase().split(/\s+/).filter(Boolean));
const holds = (parts: string[]) => {
  const text = parts.join('\n').toLowerCase();
  return words.value.every((w) => text.includes(w));
};
const shown = (m: Memory) => holds([m.title, m.body, m.evidence]);
const now = computed(() => memories.value.filter((m) => !m.ended && shown(m)));
const ended = computed(() => memories.value.filter((m) => !!m.ended && shown(m)));
const taught = computed(() => teachings.value.filter((t) => holds([t.text, t.note, t.source])));
const tabs = computed<{ value: View; label: string; count: number }[]>(() => [
  { value: 'now', label: 'Recalls now', count: now.value.length },
  { value: 'ended', label: 'No longer', count: ended.value.length },
  { value: 'teachings', label: 'Teachings', count: taught.value.length },
]);
const searching = computed(() => words.value.length > 0);
const noMatch = computed(() => `Nothing here holds “${query.value.trim()}”.`);

// The cards the last link went to, marked until the focus moves to another card. A card scrolled to stops below the tabs
// that stay over the list (--memory-bar).
const landed = ref(new Set<string>());
const root = useTemplateRef<HTMLElement>('root');
const { height: barHeight } = useElementSize(useTemplateRef<HTMLElement>('bar'), undefined, { box: 'border-box' });
async function open(tab: View, keys: string[]) {
  if (!keys.length) return;
  landed.value = new Set(keys);
  if (searching.value) query.value = '';
  view.value = tab;
  // The tab shows once the address has it, and its list after the render that follows.
  for (let tries = 0; tries < 5; tries++) {
    await nextTick();
    const el = root.value?.querySelector<HTMLElement>(`[data-key="${CSS.escape(keys[0]!)}"]`);
    if (el && el.offsetParent) {
      el.scrollIntoView({ block: 'nearest' });
      el.focus({ preventScroll: true });
      return;
    }
    await new Promise((r) => setTimeout(r, 30));
  }
}
function jump(id: number) {
  const m = find(id);
  if (m) void open(m.ended ? 'ended' : 'now', ['m' + id]);
}
function toTeachings(ids: number[]) {
  void open('teachings', ids.filter((id) => teachingById.value.has(id)).map((id) => 't' + id));
}
function onFocusIn(e: FocusEvent) {
  const card = e.target instanceof Element ? e.target.closest<HTMLElement>('[data-key]') : null;
  if (landed.value.size && !(card?.dataset.key && landed.value.has(card.dataset.key))) landed.value = new Set();
}
// A memory the stream opened: shown once the book holds it.
watch(
  [land, memoryData],
  ([id]) => {
    if (id == null || !memoryData.value) return;
    land.value = null;
    jump(id);
  },
  { immediate: true },
);

const forget = (m: Memory) => say(`Forget memory #${m.id} of ${props.who}: "${short(m.title || m.body, 120)}" because `);
</script>

<template>
  <ListState what="the memory" :held="held" :error="error" :empty="!memories.length && !teachings.length" :retry="retry">
    <template #empty><span data-memory-empty>{{ who }} remembers nothing yet. The librarian writes a memory from each teaching: what you tell it, your overturns and the replies you mark.</span></template>
    <div ref="root" class="flex flex-col" :style="{ '--memory-bar': `${barHeight}px` }" data-memory-book @focusin="onFocusIn">
      <Tabs v-model="view" class="gap-2">
        <div ref="bar" class="sticky top-0 z-10 -mt-4 bg-bg2 pt-4" data-memory-bar>
          <div class="flex flex-wrap items-center gap-2 pb-2">
            <TabsList :aria-label="`What ${who} remembers`" class="max-w-full">
              <TabsTrigger v-for="t in tabs" :key="t.value" :value="t.value" class="px-2.5" :data-memory-tab="t.value">
                {{ t.label }}<Badge variant="quiet" size="count" class="tabular-nums">{{ t.count }}</Badge>
              </TabsTrigger>
            </TabsList>
            <SearchBox v-model="query" class="ml-auto w-auto max-w-64 min-w-36 flex-1" placeholder="Search memory…" :label="`Search what ${who} remembers`" data-memory-search />
          </div>
        </div>
        <TabsContent v-show="view === 'now'" value="now" force-mount :tabindex="-1">
          <ul v-if="now.length" class="m-0 flex list-none flex-col gap-2 p-0" :aria-label="`What ${who} recalls now`" data-memory-list>
            <MemoryCard v-for="m in now" :key="m.id" :data-key="'m' + m.id" :memory="m" :find="find" :taught-in="taughtIn(m)" :replaced-by="undefined" :landed="landed.has('m' + m.id)" :forget="forget" @jump="jump" @teachings="toTeachings" />
          </ul>
          <EmptyState v-else data-memory-none>{{ searching ? noMatch : ended.length ? `${who} recalls nothing now: every memory it held was corrected or forgotten.` : `${who} recalls nothing yet: the librarian has written no memory from its teachings.` }}</EmptyState>
        </TabsContent>
        <TabsContent v-show="view === 'ended'" value="ended" force-mount :tabindex="-1">
          <ul v-if="ended.length" class="m-0 flex list-none flex-col gap-2 p-0" :aria-label="`What ${who} no longer recalls`" data-memory-list>
            <MemoryCard v-for="m in ended" :key="m.id" :data-key="'m' + m.id" :memory="m" :find="find" :taught-in="taughtIn(m)" :replaced-by="replacer.get(m.id)" :landed="landed.has('m' + m.id)" @jump="jump" @teachings="toTeachings" />
          </ul>
          <EmptyState v-else data-memory-none>{{ searching ? noMatch : 'Nothing ended yet. A memory a later one corrects, or one that was forgotten, stays here as the record of what was believed when.' }}</EmptyState>
        </TabsContent>
        <TabsContent v-show="view === 'teachings'" value="teachings" force-mount :tabindex="-1">
          <ul v-if="taught.length" class="m-0 flex list-none flex-col gap-2 p-0" :aria-label="`What ${who}'s memory learned from`" data-teaching-list>
            <TeachingCard v-for="t in taught" :key="t.id" :data-key="'t' + t.id" :teaching="t" :find="find" :landed="landed.has('t' + t.id)" @jump="jump" />
          </ul>
          <EmptyState v-else data-memory-none>{{ searching ? noMatch : 'No teaching yet: what you tell it, an overturn of yours or a reply you mark gives the librarian something to learn from.' }}</EmptyState>
        </TabsContent>
      </Tabs>
    </div>
  </ListState>
</template>
