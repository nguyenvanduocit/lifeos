<script setup lang="ts">
import { computed, ref } from 'vue'
import { GridBackground, GridItem, GridLayout, noCompactor, type GridLayoutExpose, type ReadonlyLayout } from 'grid-layout-plus'
import 'grid-layout-plus/style.css'
import type { CustomWidget } from '../core/generated/CustomWidget'
import { gridColumns, gridGap, gridRows, rowHeight } from '../widgets'

const props = defineProps<{ widgets: CustomWidget[]; editing: boolean; disabled: boolean; availableWidth: number }>()
const emit = defineEmits<{ interaction: [active: boolean] }>()
const layout = defineModel<ReadonlyLayout>('layout', { required: true })
const editor = ref<GridLayoutExpose>()
const editorWidth = computed(() => Math.max(600, props.availableWidth) + 2 * gridGap)
const cards = computed(() => {
  const ids = new Set(layout.value.map(item => item.i))
  return props.widgets.filter(widget => ids.has(widget.id))
})
const gridConfig = {
  colNum: gridColumns, rowHeight, maxRows: gridRows,
  gap: [gridGap, gridGap] as const, containerPadding: [gridGap, gridGap] as const,
}

const dragConfig = computed(() => ({ isDraggable: props.editing && !props.disabled, dragThreshold: 2 }))
const resizeConfig = computed(() => ({ isResizable: props.editing && !props.disabled }))

function changeWithKeys(event: KeyboardEvent, id: number) {
  if (!props.editing || props.disabled) return
  const item = layout.value.find(item => item.i === id)
  const steps: Record<string, [number, number]> = {
    ArrowLeft: [-1, 0], ArrowRight: [1, 0], ArrowUp: [0, -1], ArrowDown: [0, 1],
  }
  const step = steps[event.key]
  if (!item || !step) return
  event.preventDefault()
  const [dx, dy] = step
  if (event.shiftKey) editor.value?.resizeItem(id, item.w + dx, item.h + dy)
  else editor.value?.moveItem(id, item.x + dx, item.y + dy)
}
</script>

<template>
  <div :class="{ '-m-2 overflow-x-auto': editing }">
    <GridLayout ref="editor" v-model:layout="layout" class="widget-editor"
      :class="{ 'widget-view grid': !editing }"
      :style="editing ? { minWidth: `${editorWidth}px` } : {
        gap: `${gridGap}px`,
        gridTemplateColumns: availableWidth >= 600 ? `repeat(${gridColumns}, minmax(0, 1fr))` : undefined,
        gridAutoRows: availableWidth >= 600 ? `${rowHeight}px` : undefined,
      }"
      :width="editorWidth" :auto-size="editing" :grid-config="gridConfig" :compactor="noCompactor" collision-mode="push"
      :drag-config="dragConfig" :resize-config="resizeConfig"
      @interaction-start="emit('interaction', true)" @interaction-end="emit('interaction', false)">
      <GridBackground v-if="editing" color="color-mix(in srgb, var(--line2) 32%, transparent)" />
      <GridItem v-for="widget in cards" :key="widget.id" :i="widget.id"
        :style="!editing && availableWidth >= 600 ? {
          gridColumn: `${widget.grid.x + 1} / span ${widget.grid.w}`,
          gridRow: `${widget.grid.y + 1} / span ${widget.grid.h}`,
        } : undefined"
        drag-allow-from=".widget-card > header" drag-ignore-from="a, button:not(.widget-grip)" class="flex min-h-0 min-w-0">
        <slot :widget="widget" :on-keydown="(event: KeyboardEvent) => changeWithKeys(event, widget.id)" />
      </GridItem>
    </GridLayout>
  </div>
</template>

<style scoped>
.widget-editor {
  --vgl-placeholder-bg: color-mix(in srgb, var(--ink) 8%, transparent);
  --vgl-placeholder-opacity: 1;
  --vgl-item-resizing-opacity: 1;
  --vgl-resizer-size: 32px;
  --vgl-resizer-border-color: var(--ink3);
}

/* Use normal grid placement in view mode while retaining each item's box. */
.widget-view :deep(.vgl-item:not(.vgl-item--placeholder)) {
  position: static !important;
  width: auto !important;
  height: auto !important;
  transform: none !important;
}

.widget-editor :deep(.vgl-item--placeholder) {
  border: 1px dashed var(--ink3);
  border-radius: 1rem;
}

/* The next drag starts at the displayed position, including immediately after a drop. */
.widget-editor :deep(.vgl-item) {
  transition: none;
}

.widget-editor :deep(.vgl-item__resizer) {
  z-index: 1;
  touch-action: none;
}

.widget-editor :deep(.vgl-item__resizer::before) {
  inset: 8px 7px 7px 8px;
  border-radius: 0 0 4px 0;
}

.widget-editor :deep(.vgl-item--dragging .widget-card),
.widget-editor :deep(.vgl-item--resizing .widget-card) {
  border-color: var(--ink3);
  box-shadow: 0 8px 24px rgb(0 0 0 / 0.12);
}
</style>
