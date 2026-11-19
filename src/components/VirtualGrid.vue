<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import ImageCard from "./ImageCard.vue";
import { filteredIds, store } from "../store";

const CARD_WIDTH = 176;
const CARD_HEIGHT = 172;
const GAP = 12;
const OVERSCAN = 2;

const scrollElement = ref<HTMLDivElement>();
const width = ref(0);
const height = ref(600);
const scrollTop = ref(0);

const columns = computed(() =>
  Math.max(1, Math.floor((width.value + GAP) / (CARD_WIDTH + GAP))),
);
const rowHeight = CARD_HEIGHT + GAP;
const rows = computed(() => Math.ceil(filteredIds.value.length / columns.value));
const startRow = computed(() => Math.max(0, Math.floor(scrollTop.value / rowHeight) - OVERSCAN));
const endRow = computed(() =>
  Math.min(rows.value, Math.ceil((scrollTop.value + height.value) / rowHeight) + OVERSCAN),
);

const visibleRows = computed(() => {
  const result: Array<{ key: number; top: number; ids: number[] }> = [];
  for (let row = startRow.value; row < endRow.value; row += 1) {
    const ids: number[] = [];
    for (let column = 0; column < columns.value; column += 1) {
      const index = row * columns.value + column;
      const id = filteredIds.value[index];
      if (id === undefined) break;
      ids.push(id);
    }
    result.push({ key: row, top: row * rowHeight, ids });
  }
  return result;
});

function onScroll() {
  scrollTop.value = scrollElement.value?.scrollTop ?? 0;
}

function measure() {
  if (!scrollElement.value) return;
  width.value = scrollElement.value.clientWidth;
  height.value = scrollElement.value.clientHeight;
}

let observer: ResizeObserver | undefined;

onMounted(() => {
  measure();
  observer = new ResizeObserver(measure);
  if (scrollElement.value) observer.observe(scrollElement.value);
});

onBeforeUnmount(() => observer?.disconnect());
</script>

<template>
  <div ref="scrollElement" class="b-grid" @scroll="onScroll">
    <div v-if="filteredIds.length === 0" class="b-empty">
      <p>解析结果会显示在这里</p>
      <p class="muted">支持图文文章、图片消息相册、合集页与文中视频</p>
    </div>
    <div v-else class="b-grid-spacer" :style="{ height: rows * rowHeight + 'px' }">
      <div
        v-for="row in visibleRows"
        :key="row.key"
        class="b-grid-row"
        :style="{ top: row.top + 'px', gridTemplateColumns: `repeat(${columns}, 1fr)`, gap: GAP + 'px' }"
      >
        <ImageCard v-for="id in row.ids" :key="id" :view="store.views[id]" />
      </div>
    </div>
  </div>
</template>