<script lang="ts">
// The panel's icons, remit's: inline SVG strokes on one 24-unit grid, one stroke (2 units, round caps and joins), so they look the same on
// every machine, unlike symbol characters, which depend on the fonts a system has (or turn into colour emoji). Two families
// share the grid: interface marks (what a control does) and the domain's own glyphs (what a thing is), drawn for remit. ICONS is the list; a name outside it draws a plain ring rather than failing.
export const ICONS = {
  check: 'M5 12.5l4.5 4.5L19 7.5',
  plus: 'M12 5v14M5 12h14',
  spark: 'M12 3.5l1.9 5.6 5.6 1.9-5.6 1.9L12 18.5l-1.9-5.6L4.5 11l5.6-1.9z',
  alert: 'M12 4l9 16H3zM12 10v4M12 17.2v.1',
  'chevron-down': 'M6 9.5l6 6 6-6',
  'chevron-up': 'M6 14.5l6-6 6 6',
  'chevron-left': 'M14.5 6l-6 6 6 6',
  'chevron-right': 'M9.5 6l6 6-6 6',
  stop: 'M7 7h10v10H7z',
  interrupt: 'M12 4a8 8 0 1 0 0 16 8 8 0 1 0 0-16M9.5 9.5h5v5h-5z',
  archive: 'M4 5h16v4H4zM5.5 9v10h13V9M10 13h4',
  trash: 'M5 7h14M10 7V5h4v2M7 7l1 12h8l1-12M10.5 10.5v5.5M13.5 10.5v5.5',
  more: 'M6 12h.1M12 12h.1M18 12h.1',
  dot: 'M12 10.2a1.8 1.8 0 1 0 0 3.6 1.8 1.8 0 1 0 0-3.6',
  circle: 'M12 6.5a5.5 5.5 0 1 0 0 11 5.5 5.5 0 1 0 0-11',
  play: 'M8 5.5v13l10.5-6.5z',
  pause: 'M8.5 6v12M15.5 6v12',
  external: 'M14 5h5v5M19 5l-8 8M17 13.5V19H5V7h5.5',
  link: 'M10 14a4 4 0 0 0 5.7 0l3-3a4 4 0 0 0-5.7-5.7l-1.2 1.2M14 10a4 4 0 0 0-5.7 0l-3 3a4 4 0 0 0 5.7 5.7l1.2-1.2',
  eye: 'M2.5 12S6 5.5 12 5.5 21.5 12 21.5 12 18 18.5 12 18.5 2.5 12 2.5 12M12 9.5a2.5 2.5 0 1 0 0 5 2.5 2.5 0 1 0 0-5',
  clock: 'M12 4a8 8 0 1 0 0 16 8 8 0 1 0 0-16M12 8v4.5l3 2',
  copy: 'M9 9h10v10H9zM5 15V5h10',
  pencil: 'M5 19l1-4.2L15.6 5.2a2 2 0 0 1 2.8 2.8L8.8 17.6zM13.8 7l3.2 3.2',
  list: 'M9 7h10M9 12h10M9 17h10M5 7h.1M5 12h.1M5 17h.1',
  filter: 'M4 5.5h16l-6.2 7.6v5.4l-3.6 1.5v-6.9z',
  search: 'M10.5 5a5.5 5.5 0 1 0 0 11 5.5 5.5 0 1 0 0-11M14.5 14.5L19 19',
  download: 'M12 5v10M7.5 10.5L12 15l4.5-4.5M5 19h14',
  x: 'M6.5 6.5l11 11M17.5 6.5l-11 11',
  terminal: 'M5 7.5l4.5 4.5L5 16.5M12 17h7',
  file: 'M7 4h7l4 4v12H7zM14 4v4h4',
  image: 'M4 5h16v14H4zM4 16l4.5-4.5 4 4 2.5-2.5L20 18M15.5 8.5h.1',
  book: 'M5 18.5V5.5A1.5 1.5 0 0 1 6.5 4H19v13H6.5A1.5 1.5 0 0 0 5 18.5 1.5 1.5 0 0 0 6.5 20H19M9 8h6',
  gear: 'M12 9a3 3 0 1 0 0 6 3 3 0 1 0 0-6M12 3.5v2.2M12 18.3v2.2M3.5 12h2.2M18.3 12h2.2M6 6l1.6 1.6M16.4 16.4L18 18M6 18l1.6-1.6M16.4 7.6L18 6',
  'arrow-left': 'M19 12H5M11 6l-6 6 6 6',
  'arrow-right': 'M5 12h14M13 6l6 6-6 6',
  'arrow-up': 'M12 19V5M6 11l6-6 6 6',
  comment: 'M5 6h14v10H11l-4 3v-3H5z',
  help: 'M12 3.5a8.5 8.5 0 1 0 0 17 8.5 8.5 0 1 0 0-17M9.5 9.5a2.5 2.5 0 1 1 3.5 2.3c-.6.3-1 .9-1 1.6v.6M12 17v.1',
  scale: 'M12 4v16M8 20h8M5 7.5h14M5 7.5l-2.5 6a2.5 2.5 0 0 0 5 0zM19 7.5l-2.5 6a2.5 2.5 0 0 0 5 0z',
  repeat: 'M17 4l3 3-3 3M20 7H8a4 4 0 0 0-4 4M7 20l-3-3 3-3M4 17h12a4 4 0 0 0 4-4',
  key: 'M8 12.5a3.5 3.5 0 1 0 0 7 3.5 3.5 0 1 0 0-7M10.5 14l8.5-8.5M16.5 8l2.5 2.5',
  inbox: 'M3 13h5l1.5 2.5h5L16 13h5M6.2 5h11.6L21 13v6H3v-6z',
  chart: 'M5 19h14M7 15.5V11M12 15.5V6M17 15.5v-7',
  person: 'M12 4.5a3.5 3.5 0 1 0 0 7 3.5 3.5 0 1 0 0-7M5 19.5a7 7 0 0 1 14 0',
  // The domain: the Assistant a pair of sparks, an agent a head with an antenna, the crew three heads, memory a stack of records.
  assistant: 'M10 4l1.6 4.4L16 10l-4.4 1.6L10 16l-1.6-4.4L4 10l4.4-1.6zM17.5 14l.9 2.1 2.1.9-2.1.9-.9 2.1-.9-2.1-2.1-.9 2.1-.9z',
  agent: 'M6 8.5h12v10H6zM12 8.5V5M12 4v.1M9.5 13v1.5M14.5 13v1.5M3.5 12.5v3M20.5 12.5v3',
  crew: 'M12 4.5a2.5 2.5 0 1 0 0 5 2.5 2.5 0 1 0 0-5M5.5 8a2 2 0 1 0 0 4 2 2 0 1 0 0-4M18.5 8a2 2 0 1 0 0 4 2 2 0 1 0 0-4M7.5 19a4.5 4.5 0 0 1 9 0M2.5 18a3 3 0 0 1 4.5-2.6M21.5 18a3 3 0 0 0-4.5-2.6',
  memory: 'M5 6.5C5 5.1 8.1 4 12 4s7 1.1 7 2.5S15.9 9 12 9 5 7.9 5 6.5M5 6.5v11c0 1.4 3.1 2.5 7 2.5s7-1.1 7-2.5v-11M5 12c0 1.4 3.1 2.5 7 2.5s7-1.1 7-2.5',
  waits: 'M7 3.5h10M7 20.5h10M8 3.5c0 4.5 8 5 8 8.5s-8 4-8 8.5M16 3.5c0 4.5-8 5-8 8.5s8 4 8 8.5',
} as const;
export type IconName = keyof typeof ICONS;
</script>

<script setup lang="ts">
import { computed } from 'vue';

// `name` is a string, as the looks that pick one (ui.ts, the column and kind tables) hold their names as strings; a map that
// wants its names checked types them `IconName`. `label` is what a screen reader says for an icon that stands alone (no words
// beside it); without one the icon is decoration and the words beside it say it.
const props = withDefaults(defineProps<{ name: string; size?: number; label?: string }>(), { size: 14 });
const known = (name: string): name is IconName => Object.hasOwn(ICONS, name);
const path = computed(() => ICONS[known(props.name) ? props.name : 'circle']);
</script>

<template>
  <svg class="icon" :width="props.size" :height="props.size" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" :role="label ? 'img' : undefined" :aria-label="label" :aria-hidden="label ? undefined : 'true'"><path :d="path" /></svg>
</template>

<style scoped>
/* On the text baseline, never squeezed by a flex row. */
.icon {
  display: inline-block;
  flex: none;
  vertical-align: -2px;
}
</style>
