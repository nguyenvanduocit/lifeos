<script setup lang="ts">
import EntityId from '../ds/EntityId.vue'
import { computed, ref } from 'vue';
import type { Memory } from '../../core/generated/Memory';
import type { Teaching } from '../../core/generated/Teaching';
import { at, fullTime, plural, short } from './look';
import Ago from './Ago.vue';
import Icon from './Icon.vue';
import Markdown from './Markdown.vue';
import Tip from './Tip.vue';
import { Badge } from './ui/badge';
import { Button } from './ui/button';

// One memory as its holder's book shows it (MemoryBook), as remit's MemoryCard: its title over what kind it is, whether the
// owner stands behind it, who taught it, when and from which teaching; how often it was recalled and how it served; its
// body, the evidence it rests on and the memories it bears on, on asking. One its holder stopped recalling (superseded,
// forgotten) says so, when, why, and what replaced it (`replacedBy`: the memory that supersedes it). A memory or a teaching
// it names is resolved within the book (`find`, `taughtIn`) and opened by `jump` or `teachings`; `landed` marks the card a
// link went to. A forget is the owner's words to the Assistant (`forget`), offered only where the caller can say them.
const props = defineProps<{
  memory: Memory;
  find: (id: number) => Memory | undefined;
  /** The teaching it was learned from, when the book lists it. */
  taughtIn: Teaching | undefined;
  replacedBy: Memory | undefined;
  landed: boolean;
  /** Whose memory it is, said where a book holds more than one's (the Memory page). */
  holder?: string;
  forget?: (m: Memory) => void;
}>();
const emit = defineEmits<{ jump: [id: number]; teachings: [ids: number[]] }>();

const KIND: Record<string, { label: string; icon: string }> = {
  lesson: { label: 'Lesson', icon: 'book' },
  pitfall: { label: 'Pitfall', icon: 'alert' },
  preference: { label: 'Preference', icon: 'person' },
  principle: { label: 'Principle', icon: 'scale' },
  decision: { label: 'Decision', icon: 'check' },
  intent: { label: 'Intent', icon: 'arrow-right' },
  fact: { label: 'Fact', icon: 'file' },
  procedure: { label: 'Procedure', icon: 'list' },
};
const LINK: Record<string, string> = { because: 'Rests on', refines: 'Refines', contradicts: 'Contradicts', related: 'Related to', supersedes: 'Corrects' };
// The teaching it was learned from, in words.
const FROM: Record<string, string> = { teaching: 'a teaching', review: 'a review' };

const m = computed(() => props.memory);
const ended = computed(() => !!m.value.ended);
const title = computed(() => m.value.title.trim() || short(m.value.body, 120) || 'A memory with no words');
const kind = computed(() => KIND[m.value.kind] ?? { label: m.value.kind, icon: 'dot' });
const teacher = computed(() => (props.taughtIn?.teacher === 'owner' ? 'you' : ''));
const links = computed(() => m.value.links.map((l) => ({ ...l, label: LINK[l.kind] ?? l.kind, target: props.find(l.to) })));
const titleOf = (t: Memory) => t.title.trim() || short(t.body, 80) || `#${t.id}`;
const more = computed(() => !!(m.value.body.trim() || m.value.evidence.trim() || links.value.length));
const reading = ref(false);
</script>

<template>
  <li class="memory flex flex-col gap-1.5 rounded-lg px-3 py-2.5" :class="{ ended }" tabindex="-1" :data-memory-item="memory.id" :data-status="ended ? (replacedBy ? 'superseded' : 'forgotten') : 'active'">
    <div class="flex min-w-0 items-start gap-2">
      <span class="title min-w-0 flex-1 [overflow-wrap:anywhere]" data-title>{{ title }}</span>
      <EntityId :value="`memory_${memory.id}`" />
      <Tip v-if="forget && !ended" text="Forget it: starts the words in the Assistant's box, and it forgets it once you send them">
        <Button class="-my-0.5 flex-none" variant="ghost" size="icon-xs" :aria-label="`Ask the Assistant to forget the memory ${title}`" data-forget @click="forget(memory)"><Icon name="trash" :size="12" /></Button>
      </Tip>
    </div>

    <div class="flex min-w-0 flex-wrap items-center gap-x-3 gap-y-1 text-xs text-mut" data-meta>
      <Badge v-if="landed" variant="info" class="flex-none" data-landed><Icon name="arrow-right" :size="11" />Opened from a link</Badge>
      <Badge variant="quiet" class="flex-none" data-kind><Icon :name="kind.icon" :size="11" />{{ kind.label }}</Badge>
      <Badge v-if="memory.trust === 'confirmed'" variant="success" class="flex-none" data-trust="confirmed"><Icon name="check" :size="11" />Confirmed</Badge>
      <span v-if="holder" class="inline-flex min-w-0 items-center gap-1" data-holder><Icon class="flex-none" name="person" :size="11" />{{ holder }}'s</span>
      <span v-if="teacher" class="min-w-0 [overflow-wrap:anywhere]" data-teacher>taught by <span class="text-text2">{{ teacher }}</span></span>
      <span class="inline-flex min-w-0 flex-wrap items-baseline gap-x-1" data-created>
        learned <Tip :text="fullTime(at(memory.created))"><time class="rounded-sm" :datetime="at(memory.created)"><Ago :at="at(memory.created)" /></time></Tip>
        <template v-if="taughtIn">from <Tip :text="taughtIn.source"><Button variant="link" size="text" :aria-label="`from ${FROM[taughtIn.role] ?? 'a teaching'}: go to the teaching it was learned from`" data-source @click="emit('teachings', [taughtIn.id])">{{ FROM[taughtIn.role] ?? 'a teaching' }}</Button></Tip></template>
      </span>
    </div>

    <div class="flex min-w-0 flex-wrap items-center gap-x-3 gap-y-1 text-xs text-mut" data-use>
      <span v-if="memory.recalled" class="inline-flex flex-wrap items-center gap-x-1" data-recalled>
        <Icon name="repeat" :size="11" />Recalled {{ plural(memory.recalled, 'time') }}<template v-if="memory.last_recalled">, last <Tip :text="fullTime(at(memory.last_recalled))"><time class="rounded-sm" :datetime="at(memory.last_recalled)"><Ago :at="at(memory.last_recalled)" /></time></Tip></template>
      </span>
      <span v-else class="inline-flex items-center gap-1" data-recalled><Icon name="repeat" :size="11" />Not recalled yet</span>
      <span v-if="memory.helped" class="inline-flex items-center gap-1 text-text2" data-helped><Icon name="check" :size="11" />helped {{ memory.helped }}</span>
      <span v-if="memory.misled" class="inline-flex items-center gap-1 text-red-text" data-misled><Icon name="alert" :size="11" />misled {{ memory.misled }}</span>
    </div>

    <div v-if="ended" class="flex min-w-0 flex-col gap-1 text-xs text-text2" data-ended>
      <div class="flex min-w-0 flex-wrap items-center gap-x-2 gap-y-0.5">
        <Badge variant="quiet" class="flex-none" data-ended-status><Icon :name="replacedBy ? 'repeat' : 'x'" :size="11" />{{ replacedBy ? 'Superseded' : 'Forgotten' }}</Badge>
        <span v-if="memory.ended" class="text-mut"><Tip :text="fullTime(at(memory.ended))"><time class="rounded-sm" :datetime="at(memory.ended)"><Ago :at="at(memory.ended)" /></time></Tip></span>
      </div>
      <p v-if="memory.why_ended" class="m-0 min-w-0 [overflow-wrap:anywhere]" data-ended-why>{{ memory.why_ended }}</p>
      <div v-if="replacedBy" class="flex min-w-0 items-baseline gap-1.5" data-replaced>
        <Icon class="mt-0.5 flex-none self-start text-mut" name="arrow-right" :size="11" />
        <span class="flex-none text-mut">Replaced by</span>
        <Button variant="link" size="text" :aria-label="`Go to the memory that replaced it: ${titleOf(replacedBy)}`" @click="emit('jump', replacedBy.id)">{{ titleOf(replacedBy) }}</Button>
      </div>
    </div>

    <template v-if="more">
      <div v-if="reading" class="flex flex-col gap-2 pt-1">
        <Markdown v-if="memory.body.trim()" type="body" class="min-w-0" :text="memory.body" data-body />
        <blockquote v-if="memory.evidence.trim()" class="m-0 border-l-2 border-line2 pl-2.5 text-xs text-text2 [overflow-wrap:anywhere]" data-evidence>
          <span class="mb-0.5 block text-mut">Evidence, in the teaching's words</span>{{ memory.evidence }}
        </blockquote>
        <ul v-if="links.length" class="m-0 flex list-none flex-col gap-1 p-0 text-xs" aria-label="How it bears on other memories" data-links>
          <li v-for="l in links" :key="l.kind + l.to" class="flex min-w-0 items-baseline gap-1.5" :data-link="l.kind">
            <span class="flex-none text-mut">{{ l.label }}</span>
            <Button v-if="l.target" variant="link" size="text" :aria-label="`${l.label}: go to the memory ${titleOf(l.target)}`" @click="emit('jump', l.target.id)">{{ titleOf(l.target) }}</Button>
            <span v-else class="min-w-0 text-mut [overflow-wrap:anywhere]">a memory this book does not hold (<span class="mono">#{{ l.to }}</span>)</span>
          </li>
        </ul>
      </div>
      <Button class="self-start" variant="link" size="text" :aria-expanded="reading" :aria-label="`${reading ? 'Hide' : 'Read it'}: ${title}`" data-show-body @click="reading = !reading">{{ reading ? 'Hide' : 'Read it' }}<Icon :name="reading ? 'chevron-up' : 'chevron-down'" :size="11" /></Button>
    </template>
  </li>
</template>

<style scoped>
/* A memory sits on a surface, lighter than the drawer under it; whether it is still recalled is its tab, its badge and its
   quieter title. */
.memory {
  background: var(--panel2);
  scroll-margin-top: calc(var(--memory-bar, 0px) + 8px);
}
.title {
  font-weight: 500;
  color: var(--text);
}
.ended .title {
  color: var(--text2);
}
/* The card a link went to holds the focus on a lifted surface, beside its "Opened from a link" badge. */
.memory:focus {
  outline: none;
  background: var(--panel3);
}
</style>
