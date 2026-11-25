<script setup lang="ts">
import type { Question } from '../../core/generated/Question';
import { fullTime } from './look';
import Ago from './Ago.vue';
import Callout from './Callout.vue';
import FeedOverturn from './FeedOverturn.vue';
import Markdown from './Markdown.vue';
import Tip from './Tip.vue';

// A question an agent put to the owner that still waits (remit's InboxCallout, for lifeos's one kind of ask): who asks and
// when, what it asks, and where it stands. The Assistant decides each one first (remit's ADR-0087); one it is still deciding
// the owner may answer himself, one it left to him only he can: either way "Your answer" goes as his word (Call::Overturn),
// the asker gets it and the Assistant is taught. Once answered it leaves the pins as the server pushes the change.
defineProps<{ question: Question }>();
</script>

<template>
  <Callout tone="ask" icon="help" role="region" :aria-label="`${question.asker_name} asks you`" :data-question="question.id" :data-status="question.status">
    <template #title>
      <span class="flex min-w-0 items-baseline gap-2">
        <span class="min-w-0 flex-1">{{ question.asker_name }} asks you</span>
        <Tip :text="fullTime(question.created)"><time class="flex-none rounded-sm text-[11px] font-normal text-mut" :datetime="question.created"><Ago :at="question.created" /></time></Tip>
      </span>
    </template>
    <Markdown type="body" :text="question.text" />
    <p v-if="question.status === 'left'" class="m-0 mt-1 text-[12px] text-orange-text" data-left>Left to you<template v-if="question.why">: {{ question.why }}</template></p>
    <p v-else class="m-0 mt-1 text-[12px] text-mut" data-deciding>The Assistant is deciding it for you; you may answer it yourself.</p>
    <FeedOverturn :question="question" ask="Your answer" verb="Answer" data-answer />
  </Callout>
</template>
