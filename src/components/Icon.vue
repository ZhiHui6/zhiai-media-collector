<script setup lang="ts">
import { computed } from "vue";

const props = withDefaults(defineProps<{ name: string; size?: number }>(), { size: 16 });

const paths: Record<string, string> = {
  image:
    '<rect x="2.2" y="3.2" width="11.6" height="9.6" rx="1.6"/><circle cx="5.8" cy="6.6" r="1.1"/><path d="M3 11.2l3.1-3 2.6 2.4 2-1.8 2.3 2.2"/>',
  video:
    '<rect x="2.2" y="3.4" width="11.6" height="9.2" rx="2"/><path d="M6.7 6.2l3.4 1.8-3.4 1.8z"/>',
  collect:
    '<path d="M2.4 6V3.6a1.2 1.2 0 0 1 1.2-1.2H6"/><path d="M10 2.4h2.4a1.2 1.2 0 0 1 1.2 1.2V6"/><path d="M13.6 10v2.4a1.2 1.2 0 0 1-1.2 1.2H10"/><path d="M6 13.6H3.6a1.2 1.2 0 0 1-1.2-1.2V10"/><circle cx="8" cy="8" r="1.3"/>',
  clock: '<circle cx="8" cy="8" r="6.2"/><path d="M8 4.6V8l2.4 1.6"/>',
  gear:
    '<circle cx="8" cy="8" r="2.2"/><path d="M5.74 3.37 L6.31 1.21 L9.69 1.21 L10.26 3.37 L10.88 3.73 L13.04 3.14 L14.73 6.07 L13.14 7.64 L13.14 8.36 L14.73 9.93 L13.04 12.86 L10.88 12.27 L10.26 12.63 L9.69 14.79 L6.31 14.79 L5.74 12.63 L5.12 12.27 L2.96 12.86 L1.27 9.93 L2.86 8.36 L2.86 7.64 L1.27 6.07 L2.96 3.14 L5.12 3.73 Z"/>',
  folder: '<path d="M1.8 4.4h4l1.4 1.8h7v7H1.8z"/>',
  parse: '<path d="M2.4 8h8.4M8 4.8l3.2 3.2L8 11.2"/><path d="M13.2 3v10"/>',
  clear: '<path d="M4 4l8 8M12 4l-8 8"/>',
  pause: '<path d="M5.4 3.6v8.8M10.6 3.6v8.8"/>',
  play: '<path d="M5 3.4l7 4.6-7 4.6z"/>',
  retry: '<path d="M13.2 8a5.2 5.2 0 1 1-1.6-3.7"/><path d="M13.4 2.6v3.2h-3.2"/>',
  trash: '<path d="M3 4.6h10M6.4 4.6V3h3.2v1.6M4.4 4.6l.6 8.4h6l.6-8.4"/>',
  copy: '<rect x="5.4" y="5.4" width="8" height="8" rx="1.4"/><path d="M10.6 5.4V3.8a1.4 1.4 0 0 0-1.4-1.4H3.8a1.4 1.4 0 0 0-1.4 1.4v5.4a1.4 1.4 0 0 0 1.4 1.4h1.6"/>',
  down: '<path d="M8 2.6v7.6M4.8 7.4L8 10.6l3.2-3.2M2.8 13.2h10.4"/>',
  chevron: '<path d="M4 6l4 4 4-4"/>',
  back: '<path d="M9.6 3.4L5 8l4.6 4.6"/>',
  info: '<circle cx="8" cy="8" r="6.2"/><path d="M8 7.2v4.4"/><path d="M8 4.9v.1"/>',
  min: '<path d="M3 8.5h10"/>',
  max: '<rect x="3.5" y="3.5" width="9" height="9" rx="1"/>',
  code: '<path d="M5.6 3.4L2 8l3.6 4.6M10.4 3.4L14 8l-3.6 4.6"/>',
  globe:
    '<circle cx="8" cy="8" r="6.2"/><path d="M1.8 8h12.4"/><path d="M8 1.8c1.8 1.7 2.7 3.8 2.7 6.2S9.8 12.5 8 14.2c-1.8-1.7-2.7-3.8-2.7-6.2S6.2 3.5 8 1.8z"/>',
  mail: '<rect x="2" y="3.6" width="12" height="8.8" rx="1.4"/><path d="M2.4 4.4L8 8.8l5.6-4.4"/>',
  external:
    '<path d="M9.6 2.8h3.6v3.6"/><path d="M13.2 2.8L7.8 8.2"/><path d="M11.6 9.4v3.4a1.2 1.2 0 0 1-1.2 1.2H3.4a1.2 1.2 0 0 1-1.2-1.2V5.6a1.2 1.2 0 0 1 1.2-1.2h3.4"/>',
  transfer:
    '<path d="M2.6 5.6h9.2"/><path d="M9.2 3.2l2.6 2.4-2.6 2.4"/><path d="M13.4 10.4H4.2"/><path d="M6.8 8l-2.6 2.4 2.6 2.4"/>',
};
const markup = computed(() => paths[props.name] ?? "");
</script>

<template>
  <svg
    :width="size"
    :height="size"
    viewBox="0 0 16 16"
    fill="none"
    stroke="currentColor"
    stroke-width="1.7"
    stroke-linecap="round"
    stroke-linejoin="round"
    aria-hidden="true"
    v-html="markup"
  ></svg>
</template>