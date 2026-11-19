<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import Icon from "./Icon.vue";
import { api, type LibraryFile } from "../api";
import { openDirectory, removeHistoryPaths, revealPath, store } from "../store";
import { KIND_LABEL, formatBytes, kindTag, type HistoryEntry } from "../types";

interface Pack {
  dir: string;
  title: string;
  pageUrl: string;
  entries: HistoryEntry[];
  bytes: number;
  finishedAt: string;
}

const query = ref("");
const activeDir = ref<string | null>(null);
const pending = ref<Pack | null>(null);
const lightbox = ref<{ url: string; title: string; meta: string; video: boolean } | null>(null);
const files = ref<Record<string, LibraryFile>>({});
const prepared = ref(false);

function dirOf(path: string): string {
  return path.replace(/[\\/][^\\/]+$/, "") || path;
}

function baseName(path: string): string {
  const parts = path.split(/[\\/]/);
  return parts[parts.length - 1] ?? path;
}

const packs = computed<Pack[]>(() => {
  const map = new Map<string, Pack>();
  for (const entry of store.history) {
    const dir = dirOf(entry.path);
    let pack = map.get(dir);
    if (!pack) {
      pack = {
        dir,
        title: entry.page_title || baseName(dir),
        pageUrl: entry.page_url,
        entries: [],
        bytes: 0,
        finishedAt: entry.finished_at,
      };
      map.set(dir, pack);
    }
    pack.entries.push(entry);
    pack.bytes += entry.bytes;
    if (entry.finished_at > pack.finishedAt) {
      pack.finishedAt = entry.finished_at;
      pack.title = entry.page_title || baseName(dir);
      pack.pageUrl = entry.page_url;
    }
  }
  for (const pack of map.values()) {
    pack.entries.sort((a, b) => b.finished_at.localeCompare(a.finished_at));
  }
  return [...map.values()].sort((a, b) => b.finishedAt.localeCompare(a.finishedAt));
});

const filteredPacks = computed(() => {
  const keyword = query.value.trim().toLowerCase();
  if (!keyword) return packs.value;
  return packs.value.filter(
    (pack) =>
      pack.title.toLowerCase().includes(keyword) ||
      pack.dir.toLowerCase().includes(keyword),
  );
});

const active = computed(
  () => packs.value.find((pack) => pack.dir === activeDir.value) ?? null,
);

watch(
  () => store.history.length,
  async () => {
    if (store.history.length === 0) {
      files.value = {};
      prepared.value = true;
      return;
    }
    const result = await api.libraryPrepare(store.history.map((entry) => entry.path));
    const next: Record<string, LibraryFile> = {};
    for (const file of result) next[file.path] = file;
    files.value = next;
    prepared.value = true;
  },
  { immediate: true },
);

function urlOf(entry: HistoryEntry): string | null {
  const file = files.value[entry.path];
  if (!file || !file.exists) return null;
  return api.libraryUrl(file.id);
}

function coverOf(pack: Pack): string | null {
  for (const entry of pack.entries) {
    if (entry.kind === "Video") continue;
    const url = urlOf(entry);
    if (url) return url;
  }
  return null;
}

function formatTime(value: string): string {
  return value.slice(0, 16).replace("T", " ");
}

function sizeOf(entry: HistoryEntry): string {
  const file = files.value[entry.path];
  if (!file || !file.exists || !file.width) return "";
  return `${file.width} × ${file.height}`;
}

interface MenuItem {
  label: string;
  danger?: boolean;
  sep?: boolean;
  action: () => void;
}

const menu = ref<{ x: number; y: number; items: MenuItem[] } | null>(null);

function openMenuAt(event: MouseEvent, items: MenuItem[]) {
  event.preventDefault();
  menu.value = {
    x: Math.min(event.clientX, window.innerWidth - 212),
    y: Math.min(event.clientY, window.innerHeight - items.length * 34 - 20),
    items,
  };
}

async function copy(value: string) {
  try {
    await navigator.clipboard.writeText(value);
    store.status = "已复制到剪贴板";
  } catch {
    store.status = "复制失败";
  }
}

function packMenu(event: MouseEvent, pack: Pack) {
  openMenuAt(event, [
    { label: "打开素材包", action: () => (activeDir.value = pack.dir) },
    { label: "在文件夹中显示", action: () => revealPath(pack.dir) },
    { label: "复制文件夹路径", sep: true, action: () => void copy(pack.dir) },
    { label: "删除素材包…", sep: true, danger: true, action: () => (pending.value = pack) },
  ]);
}

function entryMenu(event: MouseEvent, entry: HistoryEntry) {
  openMenuAt(event, [
    { label: "预览大图", action: () => openLightbox(entry) },
    { label: "在文件夹中显示", action: () => revealPath(entry.path) },
    { label: "复制文件路径", sep: true, action: () => void copy(entry.path) },
    { label: "复制文件名", action: () => void copy(baseName(entry.path)) },
  ]);
}

function runMenu(item: MenuItem) {
  menu.value = null;
  item.action();
}

function onPointerDown(event: PointerEvent) {
  if (!menu.value) return;
  const target = event.target as HTMLElement | null;
  if (!target?.closest(".lib-ctx-menu")) menu.value = null;
}

function onKeydown(event: KeyboardEvent) {
  if (event.key !== "Escape") return;
  if (menu.value) {
    menu.value = null;
    return;
  }
  if (lightbox.value) lightbox.value = null;
}

onMounted(() => {
  window.addEventListener("keydown", onKeydown);
  window.addEventListener("pointerdown", onPointerDown);
});
onUnmounted(() => {
  window.removeEventListener("keydown", onKeydown);
  window.removeEventListener("pointerdown", onPointerDown);
});

function openLightbox(entry: HistoryEntry) {
  const url = urlOf(entry);
  if (!url) return;
  const size = sizeOf(entry);
  lightbox.value = {
    url,
    title: baseName(entry.path),
    meta: [size, entry.format, formatBytes(entry.bytes)].filter(Boolean).join(" · "),
    video: entry.kind === "Video",
  };
}

async function removePack(deleteFiles: boolean) {
  const pack = pending.value;
  if (!pack) return;
  pending.value = null;
  const paths = pack.entries.map((entry) => entry.path);
  await api.libraryDelete(paths, deleteFiles);
  removeHistoryPaths(paths);
  if (activeDir.value === pack.dir) activeDir.value = null;
  store.status = deleteFiles ? "素材包与文件已删除" : "已从素材库移除";
}
</script>

<template>
  <section class="page">
    <header class="wb-top">
      <template v-if="!active">
        <h1>素材库</h1>
        <span class="hint">{{ packs.length }} 个素材包 · {{ store.history.length }} 张</span>
        <span class="spacer"></span>
        <input
          v-model="query"
          class="field lib-search"
          aria-label="搜索素材包"
          placeholder="搜索素材包"
        />
        <button class="btn sm" @click="openDirectory(store.settings.download_root)">
          <Icon name="folder" :size="14" />打开下载目录
        </button>
      </template>
      <template v-else>
        <button class="ibtn sm" v-tip="'返回素材库'" @click="activeDir = null">
          <Icon name="back" :size="15" />
        </button>
        <h1 class="ellipsis">{{ active.title }}</h1>
        <span class="hint">{{ active.entries.length }} 张 · {{ formatBytes(active.bytes) }}</span>
        <span class="spacer"></span>
        <button class="btn sm" @click="revealPath(active.dir)">
          <Icon name="folder" :size="14" />打开目录
        </button>
        <button class="btn sm danger" @click="pending = active">
          <Icon name="trash" :size="14" />删除
        </button>
      </template>
    </header>

    <div class="page-body">
      <div v-if="packs.length === 0" class="b-empty">
        <p>素材库还是空的</p>
        <p class="muted">下载完成的图片会按文章自动归档成素材包</p>
      </div>

      <div v-else-if="!active" class="lib-grid">
        <div
          v-for="pack in filteredPacks"
          :key="pack.dir"
          class="lib-card"
          @contextmenu.prevent="packMenu($event, pack)"
        >
          <button class="lib-cover" :aria-label="pack.title" @click="activeDir = pack.dir">
            <img v-if="coverOf(pack)" :src="coverOf(pack)!" alt="" loading="lazy" />
            <span v-else class="lib-cover-empty"><Icon name="image" :size="18" /></span>
            <span class="lib-badge">{{ pack.entries.length }} 项</span>
          </button>
          <div class="lib-body">
            <div class="lib-title" v-tip="pack.title">{{ pack.title }}</div>
            <div class="lib-meta">
              <span class="hint">{{ formatTime(pack.finishedAt) }}</span>
              <span class="spacer"></span>
              <span class="hint">{{ formatBytes(pack.bytes) }}</span>
              <button class="ibtn sm" v-tip="'打开目录'" @click="revealPath(pack.dir)">
                <Icon name="folder" :size="14" />
              </button>
            </div>
          </div>
        </div>
      </div>

      <div v-else class="lib-thumbs">
        <figure
          v-for="(entry, index) in active.entries"
          :key="index"
          class="lib-item"
          @contextmenu.prevent="entryMenu($event, entry)"
        >
          <button
            class="lib-thumb"
            :aria-label="entry.page_title"
            @click="openLightbox(entry)"
          >
            <video
              v-if="entry.kind === 'Video' && urlOf(entry)"
              class="lib-thumb-video"
              :src="urlOf(entry)!"
              muted
              preload="metadata"
            ></video>
            <img v-else-if="urlOf(entry)" :src="urlOf(entry)!" alt="" loading="lazy" />
            <span v-else-if="prepared" class="lib-thumb-missing">文件缺失</span>
            <span class="lib-thumb-kind">
              <span class="tag" :class="kindTag(entry.kind)">{{ KIND_LABEL[entry.kind] }}</span>
            </span>
          </button>
          <figcaption class="lib-size">{{ sizeOf(entry) }}</figcaption>
        </figure>
      </div>
    </div>

    <div v-if="lightbox" class="lib-light">
      <button class="lib-light-close" v-tip="'关闭'" @click="lightbox = null">
        <Icon name="clear" :size="15" />
      </button>
      <video
        v-if="lightbox.video"
        class="lib-light-video"
        :src="lightbox.url"
        controls
        autoplay
      ></video>
      <img v-else :src="lightbox.url" alt="" />
      <div class="lib-light-bar">
        <span class="ellipsis">{{ lightbox.title }}</span>
        <span class="lib-light-meta">{{ lightbox.meta }}</span>
      </div>
    </div>

    <div
      v-if="menu"
      class="lib-ctx-menu"
      :style="{ left: `${menu.x}px`, top: `${menu.y}px` }"
      @pointerdown.stop
      @contextmenu.prevent
    >
      <template v-for="(item, index) in menu.items" :key="index">
        <div v-if="item.sep" class="lib-ctx-sep"></div>
        <button class="lib-ctx-item" :class="{ danger: item.danger }" @click="runMenu(item)">
          {{ item.label }}
        </button>
      </template>
    </div>

    <div v-if="pending" class="lib-dialog-scrim" @click.self="pending = null">
      <div class="lib-dialog">
        <b>删除素材包</b>
        <p class="hint">
          《{{ pending.title }}》共 {{ pending.entries.length }} 项，{{ formatBytes(pending.bytes) }}
        </p>
        <div class="lib-dialog-btns">
          <button class="btn sm" @click="pending = null">取消</button>
          <button class="btn sm" @click="removePack(false)">仅移除记录</button>
          <button class="btn sm danger" @click="removePack(true)">同时删除文件</button>
        </div>
      </div>
    </div>
  </section>
</template>
