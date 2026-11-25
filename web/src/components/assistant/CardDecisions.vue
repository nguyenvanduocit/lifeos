<script setup lang="ts">
import { computed } from 'vue';
import { useRead } from '../../data';
import { fullTime } from './look';
import Ago from './Ago.vue';
import EmptyState from './EmptyState.vue';
import FeedOverturn from './FeedOverturn.vue';
import Icon from './Icon.vue';
import Markdown from './Markdown.vue';
import Tip from './Tip.vue';

// What the Assistant decided for the owner (remit's ADR-0087 feed, AssistantFeed there): each question an agent put to the
// owner, who asked, what was answered and why. What it left to him comes first ("Needs you"), with a box for his answer;
// an answer it gave for him offers Overturn (remit's FeedOverturn): his word goes to the asker and teaches the Assistant.
// An answer he gave says so, with the answer it overturned. Read again whenever the data changes (useRead).
const { data } = useRead('list_questions', () => ({ waiting: false }));
const ORDER: Record<string, number> = { left: 0 };
const lines = computed(() => [...(data.value ?? [])].sort((a, b) => (ORDER[a.status] ?? 1) - (ORDER[b.status] ?? 1)));
const ICON: Record<string, string> = { left: 'waits', decided: 'spark', answered: 'person', open: 'clock', withdrawn: 'x' };
</script>

<template>
  <ol v-if="lines.length" class="m-0 flex list-none flex-col p-0" aria-label="What the Assistant decided for you" data-rows>
    <li v-for="q in lines" :key="q.id" class="line" :data-question="q.id" :data-status="q.status">
      <span class="ic" :class="q.status" aria-hidden="true"><Icon :name="ICON[q.status] ?? 'help'" :size="12" /></span>
      <div class="min-w-0">
        <div class="flex min-w-0 items-baseline gap-2">
          <span class="flex-none text-[11.5px] font-semibold text-violet-text">{{ q.asker_name }}</span>
          <span class="state" :class="q.status">{{ { left: 'Needs you', decided: 'Decided for you', answered: 'You answered', open: 'Deciding…', withdrawn: 'Withdrawn' }[q.status] }}</span>
          <Tip :text="fullTime(q.created)"><time class="ml-auto flex-none rounded-sm text-[11px] text-mut" :datetime="q.created"><Ago :at="q.created" /></time></Tip>
        </div>
        <p class="m-0 mt-0.5 text-text [overflow-wrap:anywhere]" :class="{ struck: q.status === 'withdrawn' }" data-text>{{ q.text }}</p>
        <p v-if="q.answer" class="answer"><Icon name="arrow-right" :size="11" /><span class="min-w-0 [overflow-wrap:anywhere]">{{ q.answer }}</span></p>
        <Markdown v-if="q.why && (q.status === 'decided' || q.status === 'left')" type="quiet" class="why" :text="(q.status === 'decided' ? 'Why: ' : 'Left to you: ') + q.why" data-why />
        <div v-if="q.overturned" class="overturn" data-overturned>
          <Icon name="interrupt" :size="12" />
          <span class="min-w-0 [overflow-wrap:anywhere]"><b class="font-semibold">Overturned:</b> {{ q.overturned }}</span>
        </div>
        <FeedOverturn v-if="q.status === 'decided'" :question="q" data-overturn />
        <FeedOverturn v-else-if="q.status === 'left'" :question="q" ask="Your answer" verb="Answer" data-answer />
      </div>
    </li>
  </ol>
  <EmptyState v-else-if="data">The Assistant has decided nothing for you yet. What it answers for you, or leaves to you, shows here.</EmptyState>
</template>

<style scoped>
.line {
  display: grid;
  grid-template-columns: 20px 1fr;
  gap: 8px;
  padding: 10px 0;
  border-bottom: 1px dashed var(--rule);
  font-size: 12.5px;
}
.ic {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 20px;
  height: 20px;
  border-radius: 5px;
  background: var(--violet-bg);
  color: var(--violet-text);
}
.ic.left {
  background: var(--orange-tint);
  color: var(--orange);
}
.ic.answered,
.ic.open,
.ic.withdrawn {
  background: var(--panel3);
  color: var(--text2);
}
.state {
  flex: none;
  font-size: 11px;
  color: var(--mut);
}
.state.left {
  color: var(--orange-text);
  font-weight: 600;
}
.struck {
  color: var(--text2);
  text-decoration: line-through;
  text-decoration-color: var(--mut);
}
.answer {
  display: flex;
  align-items: baseline;
  gap: 6px;
  margin: 4px 0 0;
  color: var(--text2);
}
.why {
  margin-top: 2px;
  font-size: 11.5px;
}
.overturn {
  display: flex;
  align-items: baseline;
  gap: 6px;
  margin-top: 6px;
  padding: 6px 8px;
  border-radius: 6px;
  background: var(--orange-bg);
  color: var(--orange-text);
}
</style>
