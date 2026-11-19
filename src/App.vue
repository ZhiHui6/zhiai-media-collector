<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted } from "vue";
import { getCurrentWindow } from "@tauri-apps/api/window";
import Icon from "./components/Icon.vue";
import VirtualGrid from "./components/VirtualGrid.vue";
import QueueBar from "./components/QueueBar.vue";
import SettingsSheet from "./components/SettingsSheet.vue";
import LibrarySheet from "./components/LibrarySheet.vue";
import TransfersSheet from "./components/TransfersSheet.vue";
import AboutSheet from "./components/AboutSheet.vue";
import PreviewSheet from "./components/PreviewSheet.vue";
import ContextMenu from "./components/ContextMenu.vue";
import {
  closePreview,
  init,
  kindCounts,
  parseAlbum,
  selectAll,
  startParse,
  store,
} from "./store";
import { tip } from "./tip";
import { KIND_LABEL, type ImageKind } from "./types";

const appWindow = getCurrentWindow();

const filters = computed<Array<{ kind: ImageKind | null; label: string; count: number }>>(() => [
  { kind: null, label: "全部", count: kindCounts.value.all ?? 0 },
  { kind: "Article", label: KIND_LABEL.Article, count: kindCounts.value.Article ?? 0 },
  { kind: "Cover", label: KIND_LABEL.Cover, count: kindCounts.value.Cover ?? 0 },
  { kind: "Avatar", label: KIND_LABEL.Avatar, count: kindCounts.value.Avatar ?? 0 },
  { kind: "QrCode", label: KIND_LABEL.QrCode, count: kindCounts.value.QrCode ?? 0 },
  { kind: "Album", label: KIND_LABEL.Album, count: kindCounts.value.Album ?? 0 },
  { kind: "Video", label: KIND_LABEL.Video, count: kindCounts.value.Video ?? 0 },
  { kind: "Unknown", label: KIND_LABEL.Unknown, count: kindCounts.value.Unknown ?? 0 },
]);

const title = computed(() => {
  const first = store.order.length > 0 ? store.views[store.order[0]] : undefined;
  return first?.item.page_title ?? "素材采集";
});

const statusDot = computed(() => {
  switch (store.parsePhase) {
    case "parsing":
      return "var(--blue)";
    case "done":
      return "var(--brand)";
    case "failed":
      return "var(--red)";
    default:
      return "var(--faint)";
  }
});

const statusWord = computed(
  () =>
    ({
      idle: "待解析",
      parsing: "解析中",
      done: "解析完成",
      failed: "解析失败",
    })[store.parsePhase],
);

function minimizeWindow() {
  void appWindow.minimize();
}

function toggleMaximize() {
  void appWindow.toggleMaximize();
}

function closeWindow() {
  void appWindow.close();
}

function onKeydown(event: KeyboardEvent) {
  if (event.key === "Escape") {
    if (store.menu) {
      store.menu = null;
      return;
    }
    if (store.previewId != null) {
      closePreview();
      return;
    }
    if (store.page) {
      store.page = null;
      return;
    }
  }
  const target = event.target as HTMLElement | null;
  const editing =
    target &&
    (target.tagName === "INPUT" || target.tagName === "TEXTAREA" || target.isContentEditable);
  if (!editing && (event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "a") {
    event.preventDefault();
    selectAll(true);
  }
}

function onGlobalClick() {
  if (store.menu) store.menu = null;
}

onMounted(() => {
  void init();
  window.addEventListener("keydown", onKeydown);
  window.addEventListener("click", onGlobalClick);
});

onBeforeUnmount(() => {
  window.removeEventListener("keydown", onKeydown);
  window.removeEventListener("click", onGlobalClick);
});
</script>

<template>
  <div class="shell">
    <header class="titlebar" data-tauri-drag-region>
      <span class="tb-title" data-tauri-drag-region>智爱媒体素材采集器</span>
      <span class="tb-ver" data-tauri-drag-region>v{{ store.version }}</span>
      <span class="spacer" data-tauri-drag-region></span>
      <div class="tb-btns">
        <button class="tb-btn" aria-label="最小化" @click="minimizeWindow">
          <Icon name="min" :size="14" />
        </button>
        <button class="tb-btn" aria-label="最大化" @click="toggleMaximize">
          <Icon name="max" :size="14" />
        </button>
        <button class="tb-btn close" aria-label="关闭" @click="closeWindow">
          <Icon name="clear" :size="14" />
        </button>
      </div>
    </header>

    <div class="app">
      <aside class="rail">
        <button class="rail-btn" :aria-selected="store.page === null" @click="store.page = null">
          <Icon name="collect" :size="18" />
          <span class="rail-label">采集</span>
        </button>
        <button
          class="rail-btn"
          :aria-selected="store.page === 'transfers'"
          @click="store.page = 'transfers'"
        >
          <Icon name="transfer" :size="18" />
          <span class="rail-label">传输</span>
        </button>
        <button
          class="rail-btn"
          :aria-selected="store.page === 'library'"
          @click="store.page = 'library'"
        >
          <Icon name="folder" :size="18" />
          <span class="rail-label">素材库</span>
        </button>
        <button
          class="rail-btn"
          :aria-selected="store.page === 'settings'"
          @click="store.page = 'settings'"
        >
          <Icon name="gear" :size="18" />
          <span class="rail-label">设置</span>
        </button>
        <button
          class="rail-btn"
          :aria-selected="store.page === 'about'"
          @click="store.page = 'about'"
        >
          <Icon name="info" :size="18" />
          <span class="rail-label">关于</span>
        </button>
        <span class="rail-grow"></span>
      </aside>

      <main class="workbench">
        <template v-if="store.page === null">
          <header class="wb-top">
            <h1 class="ellipsis">{{ title }}</h1>
            <span class="b-state">
              <i class="dot" :style="{ background: statusDot }"></i>
              {{ statusWord }}
            </span>
            <span class="wb-status ellipsis" v-tip="store.status">{{ store.status }}</span>
          </header>

          <div class="input-row">
            <input
              class="field grow"
              v-model="store.input"
              aria-label="链接输入"
              placeholder="粘贴公众号文章 / 图片消息 / 合集链接，支持文中视频，多个可用空格分隔"
              @keydown.enter="startParse"
            />
            <button class="btn pri" :disabled="!store.input.trim()" @click="startParse">
              <Icon name="parse" />解析
            </button>
            <button class="btn" :disabled="!store.input" @click="store.input = ''">清空</button>
          </div>

          <div class="chips">
            <button
              v-for="item in filters"
              :key="item.label"
              class="chip"
              :aria-pressed="store.filter === item.kind"
              @click="store.filter = item.kind"
            >
              {{ item.label }}<span class="n">{{ item.count }}</span>
            </button>
            <span class="spacer"></span>
            <span class="hint">共 {{ store.order.length }} 项</span>
          </div>

          <div v-if="store.album" class="album">
            <span class="ellipsis">
              合集《{{ store.album.title }}》包含 {{ store.album.links.length }} 篇文章
            </span>
            <span class="spacer"></span>
            <button class="btn sm" @click="store.album = null">忽略</button>
            <button class="btn sm pri" @click="parseAlbum(store.album.links)">解析全部文章</button>
          </div>

          <VirtualGrid />
          <QueueBar />
        </template>

        <SettingsSheet v-else-if="store.page === 'settings'" />
        <TransfersSheet v-else-if="store.page === 'transfers'" />
        <LibrarySheet v-else-if="store.page === 'library'" />
        <AboutSheet v-else-if="store.page === 'about'" />
      </main>
    </div>

    <PreviewSheet v-if="store.previewId != null" />
    <ContextMenu />

    <div class="loading" v-if="!store.ready">
      <span class="spinner"></span>
      正在启动…
    </div>

    <div v-if="tip.visible" class="tip" :style="{ left: `${tip.x}px`, top: `${tip.y}px` }">
      {{ tip.text }}
    </div>
  </div>
</template>
