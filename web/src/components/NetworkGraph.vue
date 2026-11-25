<script setup lang="ts">
import { computed, useId } from 'vue'

interface Vertex { id: string; label: string }
interface Edge { source: string; target: string }
const props = defineProps<{ label: string; nodes: Vertex[]; edges: Edge[] }>()
const titleId = useId()
const vertices = computed(() => {
  const result = new Map<string, Vertex & { x: number; y: number }>()
  for (const [i, vertex] of props.nodes.entries()) {
    const angle = i * 2 * Math.PI / props.nodes.length - Math.PI / 2
    result.set(vertex.id, { ...vertex, x: 240 + 157 * Math.cos(angle), y: 140 + 103 * Math.sin(angle) })
  }
  return result
})
const links = computed(() => props.edges.flatMap(edge => {
  const from = vertices.value.get(edge.source)
  const to = vertices.value.get(edge.target)
  return from && to ? [{ from, to }] : []
}))
</script>
<template>
  <figure>
    <figcaption :id="titleId" class="font-medium">{{ label }}</figcaption>
    <svg viewBox="0 0 480 280" role="img" :aria-labelledby="titleId" class="w-full">
      <line v-for="(edge, i) in links" :key="i" :x1="edge.from.x" :y1="edge.from.y" :x2="edge.to.x" :y2="edge.to.y" class="stroke-ink3" />
      <g v-for="vertex in vertices.values()" :key="vertex.id" :transform="`translate(${vertex.x} ${vertex.y})`">
        <circle r="7" class="fill-primary stroke-background" />
        <text y="23" text-anchor="middle" class="fill-current text-xs">{{ vertex.label }}</text>
      </g>
    </svg>
    <details><summary>Connections ({{ links.length }})</summary>
      <ul><li v-for="(edge, i) in links" :key="i">{{ edge.from.label }} ↔ {{ edge.to.label }}</li></ul>
    </details>
  </figure>
</template>
