<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted } from "vue";
import {
  cancelItem,
  closeMenu,
  copyText,
  downloadOne,
  openPreview,
  retryItem,
  revealPath,
  store,
} from "../store";

const view = computed(() => (store.menu ? store.views[store.menu.id] : undefined));
const itemId = computed(() => store.menu?.id ?? 0);
const url = computed(() => view.value?.item.url ?? "");
const path = computed(() => view.value?.path ?? null);
const state = computed(() => view.value?.state ?? "Queued");
const message = computed(() => view.value?.message ?? null);

const position = computed(() => {
  const menu = store.menu;
  if (!menu) return {};
  return {
    left: `${Math.min(menu.x, window.innerWidth - 200)}px`,
    top: `${Math.min(menu.y, window.innerHeight - 280)}px`,
  };
});

function run(action: () => void) {
  action();
  closeMenu();
}

let onKey: ((event: KeyboardEvent) => void) | undefined;

onMounted(() => {
  onKey = (event) => {
    if (event.key === "Escape") closeMenu();
  };
  window.addEventListener("keydown", onKey);
});

onBeforeUnmount(() => {
  if (onKey) window.removeEventListener("keydown", onKey);
});
</script>

<template>
  <div v-if="view && store.menu" class="ctx" :style="position" role="menu">
    <button class="ctx-item" @click="run(() => openPreview(itemId))">预览大图</button>
    <button class="ctx-item" @click="run(() => copyText(url))">复制原图链接</button>
    <button v-if="path" class="ctx-item" @click="run(() => path && revealPath(path))">
      打开所在目录
    </button>
    <div class="ctx-sep"></div>
    <button v-if="state !== 'Completed'" class="ctx-item" @click="run(() => downloadOne(itemId))">
      下载此张
    </button>
    <button
      v-if="state === 'Failed' || state === 'Cancelled'"
      class="ctx-item"
      @click="run(() => retryItem(itemId))"
    >
      重试
    </button>
    <button
      v-if="state === 'Queued' || state === 'Downloading'"
      class="ctx-item"
      @click="run(() => cancelItem(itemId))"
    >
      取消下载
    </button>
    <p v-if="message" class="ctx-note">{{ message }}</p>
  </div>
</template>