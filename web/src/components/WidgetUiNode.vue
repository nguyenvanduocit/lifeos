<script setup lang="ts">
import { computed } from 'vue'
import type { CustomWidget } from '../core/generated/CustomWidget'
import type { WidgetUi } from '../core/generated/WidgetUi'
import AskChip from './AskChip.vue'
import WidgetDataView from './WidgetDataView.vue'

const props = defineProps<{ elementId: string; elements: WidgetUi['elements']; widget: CustomWidget }>()
const node = computed(() => props.elements[props.elementId])
const slots = ['header', 'body', 'actions', 'footer'] as const
const slotStyle = {
  header: 'flex flex-wrap items-center gap-2',
  body: 'min-w-0 space-y-3',
  actions: 'flex flex-wrap items-center gap-2',
  footer: 'space-y-2 border-t border-line pt-2 text-ink2',
}
const gridStyle: Record<number, string> = {
  2: '@min-[20rem]/widget-ui:grid-cols-2',
  3: '@min-[20rem]/widget-ui:grid-cols-2 @min-[34rem]/widget-ui:grid-cols-3',
  4: '@min-[20rem]/widget-ui:grid-cols-2 @min-[34rem]/widget-ui:grid-cols-4',
}
</script>

<template>
  <template v-if="node">
    <section v-if="node.component === 'section'" class="min-w-0 space-y-3">
      <template v-for="slot in slots" :key="slot">
        <div v-if="node[slot]?.length" :class="slotStyle[slot]" :data-ui-slot="slot">
          <WidgetUiNode v-for="id in node[slot]" :key="id" :element-id="id" :elements="elements" :widget="widget" />
        </div>
      </template>
    </section>
    <div v-else-if="node.component === 'stack' || node.component === 'row' || node.component === 'grid'"
      class="min-w-0 gap-3" :class="node.component === 'row' ? 'flex flex-wrap items-start' : ['grid', node.component === 'grid' ? gridStyle[node.columns] : '']">
      <WidgetUiNode v-for="id in node.children" :key="id" :element-id="id" :elements="elements" :widget="widget" />
    </div>
    <h3 v-else-if="node.component === 'heading'" class="break-words text-sm font-semibold">{{ node.text }}</h3>
    <p v-else-if="node.component === 'text'" class="whitespace-pre-wrap break-words text-sm leading-relaxed">{{ node.text }}</p>
    <span v-else-if="node.component === 'badge'" class="inline-flex w-fit rounded-md border border-line bg-panel2 px-2 py-1 text-xs font-medium text-ink2">
      {{ node.text }}
    </span>
    <WidgetDataView v-else-if="node.component === 'data'" :content="node.data" :title="node.title"
      :chart-id="`widget-${widget.id}-${elementId}`" />
    <AskChip v-else-if="node.component === 'button'" :label="node.label"
      :words="`For widget #${widget.id} on LifeOS page ${JSON.stringify(widget.page)}: ${node.prompt}`" />
  </template>
</template>
