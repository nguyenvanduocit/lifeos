<script setup lang="ts">
import { computed, defineAsyncComponent, inject, reactive, ref, watch } from 'vue'
import { Check, GripVertical, LayoutGrid, Loader2, RefreshCw, Sparkles, Trash2, X } from '@lucide/vue'
import type { ReadonlyLayout } from 'grid-layout-plus'
import { useElementSize } from '@vueuse/core'
import { useRoute } from 'vue-router'
import { AlertDialogCancel, AlertDialogContent, AlertDialogDescription, AlertDialogOverlay, AlertDialogPortal, AlertDialogRoot, AlertDialogTitle } from 'reka-ui'
import type { CustomWidget } from '../core/generated/CustomWidget'
import AskChip from './AskChip.vue'
import Panel from './ds/Panel.vue'
import WidgetBody from './WidgetBody.vue'
import { Button } from './ui/button'
import { pageWidgets, type Placement } from '../widgets'

const route = useRoute()
const widgets = inject(pageWidgets)
const cards = computed(() => widgets?.value ?? [])
const layout = ref<ReadonlyLayout>()
const gridLayout = computed<ReadonlyLayout>({
  get: () => layout.value ?? cards.value.map(({ id, grid }) => ({ i: id, ...grid, minW: 8, minH: 4 })),
  set: value => {
    if (layout.value) layout.value = value
  },
})
const area = ref<HTMLElement>()
const { width: areaWidth } = useElementSize(area)
const saving = ref(false)
const interacting = ref(false)
const layoutError = ref('')
const refreshing = reactive(new Set<number>())
const refreshErrors = reactive(new Map<number, string>())
const pendingRemoval = ref<CustomWidget>()
const removing = ref(false)
const removalError = ref('')
const removalTrigger = ref<HTMLElement>()
const WidgetGridEditor = defineAsyncComponent(() => import('./WidgetGridEditor.vue'))

watch(() => route.path, () => {
  cancelLayout()
  pendingRemoval.value = undefined
})

function cancelLayout() {
  interacting.value = false
  layout.value = undefined
  layoutError.value = ''
}

async function act(action: { action: 'refresh' | 'remove'; id: number } | { action: 'arrange'; page: string; layout: Placement[] }) {
  const response = await fetch('/api/widgets', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json', 'X-LifeOS-Account': '1' },
    body: JSON.stringify(action),
  })
  if (!response.ok) {
    const body = await response.json().catch(() => null)
    throw new Error(body?.error ?? 'LifeOS could not complete this widget action.')
  }
}

function organize() {
  layoutError.value = ''
  layout.value = gridLayout.value
}

async function saveLayout() {
  const draft = layout.value
  if (!draft || saving.value || interacting.value) return
  const page = route.path.replace(/\/+$/, '') || '/'
  saving.value = true
  layoutError.value = ''
  try {
    await act({
      action: 'arrange', page,
      layout: draft.map(({ i, x, y, w, h }) => ({ id: Number(i), grid: { x, y, w, h } })),
    })
    if (layout.value === draft) cancelLayout()
  } catch (err) {
    if (layout.value === draft) layoutError.value = err instanceof Error ? err.message : String(err)
  } finally {
    saving.value = false
  }
}

function confirmRemoval(widget: CustomWidget, event: MouseEvent) {
  removalError.value = ''
  removalTrigger.value = event.currentTarget as HTMLElement
  pendingRemoval.value = widget
}

async function removeWidget() {
  const widget = pendingRemoval.value
  if (!widget || removing.value) return
  removing.value = true
  try {
    await act({ action: 'remove', id: widget.id })
    if (pendingRemoval.value?.id === widget.id) pendingRemoval.value = undefined
  } catch (err) {
    removalError.value = err instanceof Error ? err.message : String(err)
  } finally {
    removing.value = false
  }
}

function restoreRemovalFocus(event: Event) {
  event.preventDefault()
  if (removalTrigger.value?.isConnected) removalTrigger.value.focus()
}

async function refresh(widget: CustomWidget) {
  refreshing.add(widget.id)
  refreshErrors.delete(widget.id)
  try {
    await act({ action: 'refresh', id: widget.id })
  } catch (err) {
    refreshErrors.set(widget.id, err instanceof Error ? err.message : String(err))
  } finally {
    refreshing.delete(widget.id)
  }
}
</script>

<template>
  <section v-if="cards.length || layout" aria-label="Custom widgets">
    <Teleport to="#page-actions">
      <template v-if="layout">
        <Button size="sm" variant="ghost" class="rounded-lg text-[13px]" :disabled="saving" @click="cancelLayout"><X class="size-3.5" />Cancel</Button>
        <Button size="sm" class="rounded-lg text-[13px]" :disabled="saving || interacting" @click="saveLayout">
          <Check class="size-3.5" />Save layout
        </Button>
      </template>
      <Button v-else size="lg" variant="outline" class="rounded-xl border-line2" @click="organize">
        <LayoutGrid class="size-4" />Organize
      </Button>
    </Teleport>
    <p v-if="layoutError" role="alert" class="mb-2 text-sm text-critical">{{ layoutError }}</p>
    <div ref="area" :class="{ organizing: layout }"
      @selectstart="layout && $event.preventDefault()" @dragstart="layout && $event.preventDefault()">
      <span v-if="layout" id="widget-grid-instructions" class="sr-only">
        Use the arrow keys to move the widget. Hold Shift to resize.
      </span>
      <WidgetGridEditor v-model:layout="gridLayout" :editing="!!layout" :disabled="saving" :widgets="cards"
        :available-width="areaWidth" @interaction="interacting = $event">
        <template #default="{ widget, onKeydown }">
          <Panel :title="widget.title" :icon="Sparkles" pad
            :data-widget-id="widget.id"
            class="widget-card flex min-h-0 flex-1 flex-col [&>div]:min-h-0 [&>div]:overflow-auto [&>header]:shrink-0"
            :class="{ 'shadow-sm [&>header]:cursor-grab [&>header]:touch-none [&>header]:select-none': layout }">
            <template #actions>
              <button v-if="layout" type="button" class="widget-grip cursor-grab rounded-md p-1.5 text-ink3 hover:bg-panel3 hover:text-ink"
                :aria-label="`Move ${widget.title}`" aria-describedby="widget-grid-instructions"
                :disabled="saving" @keydown="onKeydown">
                <GripVertical class="size-4" />
              </button>
              <template v-else>
                <Button size="icon" variant="ghost" class="size-8" :disabled="refreshing.has(widget.id)"
                  :aria-label="`Update now · ${widget.title}`" title="Update now" @click="refresh(widget)">
                  <Loader2 v-if="refreshing.has(widget.id)" class="size-4 motion-safe:animate-spin" />
                  <RefreshCw v-else class="size-4" />
                </Button>
                <AskChip ghost label="Edit widget" :words="`Update widget widget_${widget.id}: `" />
                <Button size="icon" variant="ghost" class="size-8 text-ink3 hover:text-critical"
                  :aria-label="`Delete widget · ${widget.title}`" title="Delete widget" @click="confirmRemoval(widget, $event)">
                  <Trash2 class="size-4" />
                </Button>
              </template>
            </template>
            <div :inert="!!layout"><WidgetBody :widget="widget" /></div>
            <p v-if="refreshing.has(widget.id)" role="status" class="mt-3 text-xs text-ink2">Updating…</p>
            <p v-if="refreshErrors.has(widget.id)" role="alert" class="mt-3 text-sm text-critical">{{ refreshErrors.get(widget.id) }}</p>
          </Panel>
        </template>
      </WidgetGridEditor>
    </div>
    <AlertDialogRoot :open="!!pendingRemoval"
      @update:open="open => { if (!open && !removing) pendingRemoval = undefined }">
      <AlertDialogPortal>
        <AlertDialogOverlay class="fixed inset-0 z-50 bg-black/40" />
        <AlertDialogContent
          class="fixed top-1/2 left-1/2 z-50 w-[calc(100%-2rem)] max-w-sm -translate-x-1/2 -translate-y-1/2 rounded-2xl border border-line bg-panel p-5 shadow-xl"
          @close-auto-focus="restoreRemovalFocus" @escape-key-down="removing && $event.preventDefault()">
          <AlertDialogTitle class="text-base font-semibold">Delete widget?</AlertDialogTitle>
          <AlertDialogDescription class="mt-2 text-sm text-ink2">“{{ pendingRemoval?.title }}” will be removed from this page.</AlertDialogDescription>
          <p v-if="removalError" role="alert" class="mt-3 text-sm text-critical">{{ removalError }}</p>
          <div class="mt-5 flex justify-end gap-2">
            <AlertDialogCancel as-child><Button variant="outline" :disabled="removing">Cancel</Button></AlertDialogCancel>
            <Button variant="destructive" :disabled="removing" @click="removeWidget">{{ removing ? 'Deleting…' : 'Delete widget' }}</Button>
          </div>
        </AlertDialogContent>
      </AlertDialogPortal>
    </AlertDialogRoot>
  </section>
</template>


<style scoped>
.organizing,
.organizing :deep(*) {
  -webkit-user-select: none;
  user-select: none;
}
</style>
