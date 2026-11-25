<script setup lang="ts">
import type { CustomWidget } from '../core/generated/CustomWidget'
import Markdown from './Markdown.vue'
import WidgetDataView from './WidgetDataView.vue'
import WidgetUiNode from './WidgetUiNode.vue'

defineProps<{ widget: CustomWidget }>()
</script>

<template>
  <Markdown v-if="!widget.content" :source="widget.markdown" class="max-w-full overflow-x-auto break-words text-sm" />
  <div v-else-if="widget.content.kind === 'ui'" class="@container/widget-ui">
    <WidgetUiNode :element-id="widget.content.root" :elements="widget.content.elements" :widget="widget" />
  </div>
  <WidgetDataView v-else :content="widget.content" :title="widget.title" :chart-id="`widget-${widget.id}`" />
</template>
