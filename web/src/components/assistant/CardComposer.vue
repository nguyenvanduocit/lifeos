<script setup lang="ts">
import { computed, ref, useId, watch } from 'vue';
import { agentState, interrupt, messageOf, panel, restart, send, setDraft, setImages } from '../../assistant';
import { stop as stopAgent } from '../../core/link';
import { fileName, imageUrl, isFile, putAttachment } from '../../images';
import type { AgentStanding } from '../../core/generated/AgentStanding';
import type { Ask } from '../../core/generated/Ask';
import type { Card } from '../../core/generated/Card';
import type { Delivery } from '../../core/generated/Delivery';
import { Button } from './ui/button';
import { Textarea } from './ui/textarea';
import { onComposerKey } from './composerKeys';
import Icon from './Icon.vue';
import Tip from './Tip.vue';
import Spinner from './Spinner.vue';
import ReasonDialog from './ReasonDialog.vue';

// The box to message the agent, with what the server says the owner can ask of it (`agent.asks`), as remit's is. What is
// typed is the agent's draft, which the server keeps as it is typed: the same in every tab, and there after a reload.
// Under the box, what Send does now in the server's words (`agent.on_send`: it waits for the turn in progress) when the
// agent does not read it at once, and the sends on the right: Send queues the message (read after the turn in progress, at
// once when none runs), and while a turn runs the server may offer Steer (into that turn) and Interrupt (the turn ends and
// the message is read next). With nothing typed during a turn, Interrupt alone ends it. While there is something to stop
// (a turn, messages waiting), Stop agent… asks for a why first (ReasonDialog) and stops it (remit's cardStop). Attachments are saved with the draft, and Send waits for uploads to finish.
const props = defineProps<{ card: Card; name: string; agent: AgentStanding }>();
const emit = defineEmits<{ sent: [] }>();
const can = (a: Ask) => props.agent.asks.includes(a);
const writes = computed(() => can('message'));
const state = computed(() => agentState(props.card.id));
const draft = computed({ get: () => state.value.draft, set: (v: string) => setDraft(props.card.id, v) });
const images = computed({ get: () => state.value.images, set: (v) => setImages(props.card.id, v) });
const unsent = computed(() => draft.value.trim() !== '' || images.value.length > 0);
const picker = ref<HTMLInputElement>();
const putting = ref(0);
const notAdded = ref('');
async function attach(files: readonly File[]) {
  if (!files.length) return;
  const card = props.card.id;
  putting.value += files.length;
  notAdded.value = '';
  for (const file of files) {
    try {
      const attachment = await putAttachment(file);
      setImages(card, [...agentState(card).images, attachment]);
    } catch (e) {
      notAdded.value += `${notAdded.value ? ' ' : ''}Could not attach ${file.name}. ${messageOf(e)}`;
    } finally {
      putting.value--;
    }
  }
}
function onPaste(e: ClipboardEvent) {
  const files = Array.from(e.clipboardData?.files ?? []);
  if (!files.length) return;
  e.preventDefault();
  void attach(files);
}
function onDrop(e: DragEvent) {
  const files = Array.from(e.dataTransfer?.files ?? []);
  if (files.length) void attach(files);
}
function onPick(e: Event) {
  const input = e.target as HTMLInputElement;
  void attach(Array.from(input.files ?? []));
  input.value = '';
}
const unattach = (id: string) => (images.value = images.value.filter((i) => i.id !== id));
function sendAs(how: Delivery) {
  if (!unsent.value || putting.value) return;
  send(props.card.id, how);
  emit('sent');
}
const saysId = useId();
// Stop shows when the server lets the owner stop the agent, and asks first, with what a stop does in the server's words
// (`agent.stop_says`, empty when he may not) and a why, which stays in the agent's stream for it to read next.
const confirmStop = ref(false);
const stopping = ref(false);
const stopFailed = ref('');
watch(confirmStop, (o) => o && (stopFailed.value = ''));
async function stop(why: string) {
  stopping.value = true;
  try {
    await stopAgent(props.card.id, why);
    confirmStop.value = false;
  } catch (e) {
    stopFailed.value = `Could not stop ${props.name}: ${messageOf(e)}`;
  } finally {
    stopping.value = false;
  }
}
// A page that put words in the box (say) gives it the focus, at the end of the words.
const box = ref<{ $el: HTMLTextAreaElement } | null>(null);
watch(
  () => panel.focusTick,
  () => {
    const el = box.value?.$el;
    if (!el) return;
    el.focus();
    el.setSelectionRange(el.value.length, el.value.length);
  },
  { flush: 'post' },
);
</script>

<template>
  <div v-if="!writes" class="flex items-center justify-between gap-1.5 border-t border-line px-3 py-2.5">
    <span v-if="can('restart')" class="text-xs text-mut">{{ name }} has no session to resume.</span>
    <span v-else class="text-xs text-mut">{{ name }} takes no more messages.</span>
    <Tip v-if="can('restart')" :text="`Start ${name} again`"><Button variant="outline" size="sm" @click="restart(card.id)">Restart</Button></Tip>
    <Tip v-if="agent.stop_says" :text="agent.stop_says"><Button variant="destructive" size="sm" @click="confirmStop = true">Stop agent…</Button></Tip>
  </div>
  <div v-else class="border-t border-line px-3 pt-2.5 pb-2" data-composer>
    <div
      class="rounded-xl border border-line2 bg-panel2 shadow-[var(--shadow-card)] transition-[border-color,box-shadow] focus-within:border-blue-line focus-within:ring-3 focus-within:ring-blue-tint"
      @dragover.prevent
      @drop.prevent="onDrop"
    >
      <ul v-if="images.length || putting" class="flex flex-wrap gap-2 px-3 pt-2.5" aria-label="Attachments in this message" data-images>
        <li v-for="(i, n) in images" :key="i.id" class="shot relative max-w-full">
          <span v-if="isFile(i)" class="flex h-14 max-w-56 items-center gap-1.5 rounded-md border border-line px-2 text-xs" :title="fileName(i)">
            <Icon name="file" :size="14" class="shrink-0" /><span class="truncate">{{ fileName(i) }}</span>
          </span>
          <img v-else :src="imageUrl(i.id)" :alt="`Image ${n + 1}`" class="size-14 rounded-md border border-line object-cover" />
          <button type="button" class="unattach" :aria-label="isFile(i) ? `Remove ${fileName(i)}` : `Remove image ${n + 1}`" @click="unattach(i.id)"><Icon name="x" :size="10" /></button>
        </li>
        <li v-if="putting" class="flex size-14 items-center justify-center rounded-md border border-dashed border-line2 text-mut" aria-live="polite"><Spinner :size="14" /><span class="sr-only">Adding attachments…</span></li>
      </ul>
      <Textarea
        ref="box"
        v-model="draft"
        rows="1"
        :aria-label="`Message ${name}`"
        :aria-describedby="agent.on_send ? saysId : undefined"
        :placeholder="`Message ${name}…`"
        class="max-h-48 min-h-0 resize-none rounded-none border-0 bg-transparent px-3 pt-2.5 pb-1 text-[13px] leading-relaxed shadow-none outline-none focus-visible:outline-none focus-visible:ring-0 disabled:bg-transparent disabled:opacity-100"
        @keydown="onComposerKey($event, () => sendAs('queue'), () => ($event.target as HTMLElement).blur())"
        @paste="onPaste"
      />
      <p v-if="notAdded" class="px-3 pb-1 text-[11.5px] text-red-text" role="alert" data-not-added>{{ notAdded }}</p>
      <div class="flex flex-wrap items-center gap-1.5 px-2 pt-1 pb-2">
        <input ref="picker" type="file" multiple class="hidden" @change="onPick" />
        <Tip text="Attach files (or paste or drop them in the box)">
          <Button variant="ghost" size="icon-sm" class="text-mut" aria-label="Attach files" data-attach @click="picker?.click()"><Icon name="file" :size="14" /></Button>
        </Tip>
        <Tip v-if="agent.stop_says" :text="agent.stop_says">
          <Button variant="ghost" size="sm" class="text-mut hover:bg-red-tint hover:text-red-text remit:hover:bg-red-tint" data-stop @click="confirmStop = true"><Icon name="stop" :size="12" />Stop agent…</Button>
        </Tip>
        <!-- In the button row, which is always there, so the words coming and going with a turn never move the page. -->
        <span v-if="agent.on_send" :id="saysId" class="flex min-w-0 flex-1 items-center gap-1.5 px-1 text-[11.5px] text-mut" :title="agent.on_send" data-on-send><Icon class="flex-none" name="clock" :size="12" /><span class="truncate">{{ agent.on_send }}</span></span>
        <div class="ml-auto flex flex-wrap justify-end gap-1.5" data-sends>
          <Tip v-if="can('interrupt') && !unsent" text="End the turn in progress. The agent stays available for your next message.">
            <Button variant="outline" size="sm" class="rounded-full" @click="interrupt(card.id)"><Icon name="interrupt" :size="13" />Interrupt</Button>
          </Tip>
          <template v-if="unsent">
            <Tip v-if="can('steer')" text="Put this into the turn in progress: the agent reads it at its next step"><Button variant="outline" size="sm" class="rounded-full" data-send="steer" @click="sendAs('steer')">Steer</Button></Tip>
            <Tip v-if="can('interrupt')" text="End the turn in progress: the agent reads this next"><Button variant="outline" size="sm" class="rounded-full" data-send="interrupt" @click="sendAs('interrupt')">Interrupt and send</Button></Tip>
          </template>
          <Button v-if="unsent || !can('interrupt')" size="sm" class="rounded-full pl-3" :disabled="!unsent || !!putting" data-send="queue" @click="sendAs('queue')">Send<Icon name="arrow-up" :size="13" /></Button>
        </div>
      </div>
    </div>
  </div>
  <ReasonDialog v-model:open="confirmStop" :title="`Stop ${name}?`" :description="agent.stop_says" verb="Stop agent" busy="Stopping…" :running="stopping" :error="stopFailed" @confirm="stop" />
</template>

<style scoped>
.unattach {
  position: absolute;
  top: -6px;
  right: -6px;
  display: flex;
  width: 18px;
  height: 18px;
  align-items: center;
  justify-content: center;
  border: 1px solid var(--line2);
  border-radius: 9999px;
  background: var(--panel);
  color: var(--mut);
}
.unattach:hover {
  color: var(--text);
}
.unattach:focus-visible {
  outline: 2px solid var(--blue-line);
  outline-offset: 1px;
}
</style>
