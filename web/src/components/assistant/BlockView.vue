<script setup lang="ts">
import { computed, reactive, ref } from 'vue';
import { messageOf } from '../../assistant';
import { blockPick } from '../../core/link';
import type { Block } from '../../core/generated/Block';
import type { BlockPart } from '../../core/generated/BlockPart';
import type { EventRead } from '../../core/generated/EventRead';
import { actorName } from './stream';
import { useStream } from './streamContext';
import Ago from './Ago.vue';
import BlockChart from './BlockChart.vue';
import BlockChoice from './BlockChoice.vue';
import Icon from './Icon.vue';
import Spinner from './Spinner.vue';

// A block an agent showed in its stream (`show_block`, remit's ADR-0091): its title, its parts drawn by the panel's own
// components (a choice, a chart), and whom a pick tells. A pick is the owner's message to that agent (`blockPick`); the
// block shows the picks the server holds and who picked last, or, while a pick is on its way, that pick and that it is
// telling the agent. The agent never waits on it: until he picks, it goes on with its own.
const props = defineProps<{ event: EventRead; block: Block }>();
const stream = useStream();
const agent = computed(() => stream.nameOf(props.event.event.card));
const hasChoice = computed(() => props.block.spec.parts.some((p) => 'choice' in p));

// The picks on their way, by part: the one sent and the newest made meanwhile, which goes when it is answered, so a run of
// clicks ends on the last. Each part has its own, so a pick in one choice never stands in for another's. A choice shows,
// and builds its next pick from, the newest of its own still on its way, else the server's.
type Choice = Extract<BlockPart, { choice: unknown }>['choice'];
const ahead = reactive(new Map<string, { sending: string[]; next?: string[] }>());
const picking = computed(() => ahead.size > 0);
// The panel has no toasts: a pick that did not go says so under the block until the next one goes.
const failed = ref('');
const newest = (part: Choice) => {
  const a = ahead.get(part.id);
  return a?.next ?? a?.sending ?? part.picked;
};
async function pick(part: string, picked: string[]) {
  const a = ahead.get(part);
  if (a) {
    a.next = picked;
    return;
  }
  failed.value = '';
  for (let send: string[] | undefined = picked; send; send = ahead.get(part)?.next) {
    ahead.set(part, { sending: send });
    try {
      await blockPick(props.event.event.id, part, send);
    } catch (e) {
      failed.value = `Could not tell ${agent.value}: ${messageOf(e)}`;
    }
  }
  ahead.delete(part);
}
</script>

<template>
  <section class="block" :aria-label="block.spec.title" :data-block="event.event.id">
    <h4 class="title"><Icon :name="hasChoice ? 'list' : 'chart'" :size="13" class="flex-none text-mut" />{{ block.spec.title }}</h4>
    <div class="flex flex-col gap-4">
      <template v-for="(part, n) in block.spec.parts" :key="n">
        <BlockChoice v-if="'choice' in part" :part="part.choice" :picked="newest(part.choice)" @pick="(picked) => pick(part.choice.id, picked)" />
        <BlockChart v-else :part="part.chart" />
      </template>
    </div>
    <footer v-if="hasChoice" class="foot" role="status">
      <template v-if="picking"><Spinner />Telling {{ agent }}…</template>
      <span v-else-if="failed" class="text-red-text">{{ failed }}</span>
      <span v-else-if="block.picked_by">{{ actorName(block.picked_by, stream.nameOf) }} picked<template v-if="block.picked_at">&nbsp;<Ago :at="block.picked_at" /></template>; {{ agent }} is told</span>
      <template v-else>The agent's own pick, which it goes on with; your pick tells {{ agent }}</template>
    </footer>
  </section>
</template>

<style scoped>
/* A block: a card of its own in the stream, on the panel's surface with the card's half-pixel ring; its options sit on
   the surface above it. */
.block {
  padding: 10px 12px;
  border-radius: 8px;
  background: var(--panel);
  box-shadow: var(--shadow-card);
}
.title {
  display: flex;
  align-items: center;
  gap: 6px;
  margin: 0 0 10px;
  color: var(--text);
  font-size: var(--size-read);
  font-weight: 600;
  line-height: var(--leading-ui);
}
.foot {
  display: flex;
  align-items: center;
  gap: 6px;
  margin-top: 10px;
  color: var(--mut);
  font-size: var(--size-meta);
}
</style>
