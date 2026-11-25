<script setup lang="ts">
import { computed } from 'vue'
import { parseUiDocument } from '../../jsonUi'
import WidgetUiNode from '../WidgetUiNode.vue'

const props = defineProps<{ document: string; id: string }>()
const tree = computed(() => parseUiDocument(JSON.parse(props.document)))
</script>
<template>
  <WidgetUiNode :element-id="tree.root" :elements="tree.elements" :widget="{ id, page: $route.path }" />
</template>
