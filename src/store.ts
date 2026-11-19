import { computed, reactive } from "vue";
import { api, onCoreEvent } from "./api";
import type {
  CoreEvent,
  ImageItem,
  ImageKind,
  ItemState,
  QueueStats,
  Settings,
  ThemeMode,
} from "./types";

export interface View {
  item: ImageItem;
  state: ItemState;
  message: string | null;
  done: number;
  total: number;
  speed: number;
  lastSample: { done: number; at: number } | null;
  path: string | null;
  bytes: number | null;
  format: string | null;
  thumbUrl: string | null;
  thumbWidth: number;
  thumbHeight: number;
}

const DEFAULT_SETTINGS: Settings = {
  theme: "Dark",
  download_root: "",
  concurrency: 4,
  retry_max: 3,
  convert_enabled: false,
  convert_target: "Jpg",
  jpeg_quality: 92,
  webp_quality: 85,
};

export const store = reactive({
  ready: false,
  version: "",
  input: "",
  parsePhase: "idle" as "idle" | "parsing" | "done" | "failed",
  status: "粘贴链接后点击解析",
  order: [] as number[],
  views: {} as Record<number, View>,
  filter: null as ImageKind | null,
  queueIds: [] as number[],
  stats: {
    queued: 0,
    active: 0,
    completed: 0,
    failed: 0,
    paused: false,
    concurrency: 4,
  } as QueueStats,
  settings: { ...DEFAULT_SETTINGS } as Settings,
  history: [] as Array<import("./types").HistoryEntry>,
  album: null as { title: string; links: string[] } | null,
  page: null as "settings" | "library" | "about" | "transfers" | null,
  previewId: null as number | null,
  previewUrl: null as string | null,
  previewError: null as string | null,
  previewSize: { width: 0, height: 0 },
  queuePinned: false,
  queueOpen: false,
  menu: null as { x: number; y: number; id: number } | null,
});

const seen = new Set<string>();

export const filteredIds = computed(() => {
  if (!store.filter) return store.order;
  return store.order.filter((id) => store.views[id]?.item.kind === store.filter);
});

export const selectedCount = computed(
  () => store.order.filter((id) => store.views[id]?.item.selected).length,
);

export const kindCounts = computed(() => {
  const counts: Record<string, number> = { all: store.order.length };
  for (const id of store.order) {
    const kind = store.views[id]?.item.kind;
    if (kind) counts[kind] = (counts[kind] ?? 0) + 1;
  }
  return counts;
});

export const queueExpanded = computed(() =>
  store.queuePinned ? store.queueOpen : store.stats.active > 0 || store.stats.queued > 0,
);

export const totalSpeed = computed(() =>
  store.order.reduce((sum, id) => {
    const view = store.views[id];
    return view?.state === "Downloading" ? sum + view.speed : sum;
  }, 0),
);

export const queueViews = computed(() =>
  store.queueIds
    .map((id) => store.views[id])
    .filter((view): view is View => Boolean(view)),
);

function makeView(item: ImageItem): View {
  return {
    item,
    state: "Queued",
    message: null,
    done: 0,
    total: 0,
    speed: 0,
    lastSample: null,
    path: null,
    bytes: null,
    format: null,
    thumbUrl: null,
    thumbWidth: 0,
    thumbHeight: 0,
  };
}

function dedupKey(url: string): string {
  try {
    const parsed = new URL(url);
    return `${parsed.host}${parsed.pathname}`;
  } catch {
    return url;
  }
}

function handleEvent(event: CoreEvent) {
  switch (event.type) {
    case "parseStarted": {
      store.parsePhase = "parsing";
      store.status = `正在解析：${event.url}`;
      break;
    }
    case "batchParsed": {
      const outcome = event.outcome;
      if ("Images" in outcome) {
        const batch = outcome.Images;
        let added = 0;
        for (const item of batch.items) {
          const key = dedupKey(item.url);
          if (seen.has(key)) continue;
          seen.add(key);
          store.order.push(item.id);
          store.views[item.id] = makeView(item);
          added += 1;
        }
        let status = `解析完成：《${batch.page_title}》新增 ${added} 张`;
        if (added < batch.items.length) status += `（去重 ${batch.items.length - added} 张）`;
        if (batch.warnings.length) status += `（${batch.warnings.join("；")}）`;
        store.status = status;
        store.parsePhase = "done";
      } else if ("ArticleLinks" in outcome) {
        const album = outcome.ArticleLinks;
        store.album = { title: album.title, links: album.links };
        store.status = `合集《${album.title}》包含 ${album.links.length} 篇文章`;
        store.parsePhase = "done";
      } else {
        store.parsePhase = "failed";
        store.status = outcome.Failed.message;
      }
      break;
    }
    case "itemState": {
      const view = store.views[event.id];
      if (!view) break;
      view.state = event.state;
      view.message = event.message;
      if (event.state === "Queued") view.speed = 0;
      break;
    }
    case "itemProgress": {
      const view = store.views[event.id];
      if (!view) break;
      const now = performance.now();
      if (view.lastSample && now - view.lastSample.at >= 200) {
        const delta = event.done - view.lastSample.done;
        if (delta > 0) view.speed = (delta * 1000) / (now - view.lastSample.at);
        view.lastSample = { done: event.done, at: now };
      } else if (!view.lastSample) {
        view.lastSample = { done: event.done, at: now };
      }
      view.done = event.done;
      view.total = event.total;
      break;
    }
    case "itemCompleted": {
      const view = store.views[event.id];
      if (!view) break;
      view.path = event.path;
      view.bytes = event.bytes;
      view.format = event.format;
      view.speed = 0;
      break;
    }
    case "thumbReady": {
      const view = store.views[event.id];
      if (!view) break;
      view.thumbWidth = event.width;
      view.thumbHeight = event.height;
      view.thumbUrl = api.mediaUrl("thumb", event.id);
      break;
    }
    case "fullReady": {
      if (store.previewId !== event.id) break;
      store.previewSize = { width: event.width, height: event.height };
      store.previewError = null;
      store.previewUrl = api.mediaUrl("full", event.id);
      break;
    }
    case "fullFailed": {
      if (store.previewId !== event.id) break;
      store.previewError = event.message;
      break;
    }
    case "queueStats": {
      store.stats = event.stats;
      break;
    }
    case "history": {
      store.history.push(event.entry);
      break;
    }
  }
}

function isDark(mode: ThemeMode): boolean {
  return (
    mode === "Dark" ||
    (mode === "System" && window.matchMedia("(prefers-color-scheme: dark)").matches)
  );
}

export function syncDocumentTheme() {
  document.documentElement.dataset.theme = isDark(store.settings.theme) ? "dark" : "light";
}

let saveTimer: number | undefined;

export function saveSettingsSoon() {
  window.clearTimeout(saveTimer);
  saveTimer = window.setTimeout(() => {
    void api.updateSettings({ ...store.settings });
  }, 300);
}

export function saveSettingsNow() {
  window.clearTimeout(saveTimer);
  void api.updateSettings({ ...store.settings });
}

export function setTheme(mode: ThemeMode) {
  store.settings.theme = mode;
  syncDocumentTheme();
  void api.applyTheme(mode.toLowerCase());
  saveSettingsSoon();
}

export async function init() {
  const data = await api.bootstrap();
  store.version = data.version;
  store.settings = { ...DEFAULT_SETTINGS, ...data.settings };
  store.history = data.history;
  store.stats = { ...store.stats, ...data.stats };
  syncDocumentTheme();
  await onCoreEvent(handleEvent);
  window
    .matchMedia("(prefers-color-scheme: dark)")
    .addEventListener("change", () => {
      if (store.settings.theme === "System") syncDocumentTheme();
    });
  store.ready = true;
}

export function startParse() {
  const text = store.input;
  if (!text.trim()) return;
  store.input = "";
  void api.parseText(text);
}

export function parseAlbum(links: string[]) {
  store.album = null;
  store.parsePhase = "parsing";
  store.status = `正在解析合集内 ${links.length} 篇文章…`;
  void api.parseAlbum(links);
}

export function toggleSelect(id: number) {
  const view = store.views[id];
  if (view) view.item.selected = !view.item.selected;
}

export function selectAll(value: boolean) {
  for (const id of filteredIds.value) {
    const view = store.views[id];
    if (view) view.item.selected = value;
  }
}

export function invertSelection() {
  for (const id of filteredIds.value) {
    const view = store.views[id];
    if (view) view.item.selected = !view.item.selected;
  }
}

function queueIds(ids: number[]) {
  for (const id of ids) {
    if (!store.queueIds.includes(id)) store.queueIds.push(id);
  }
  store.queuePinned = true;
  store.queueOpen = true;
}

export function downloadSelected() {
  const ids = filteredIds.value.filter((id) => store.views[id]?.item.selected);
  if (ids.length === 0) {
    store.status = "请先勾选要下载的内容";
    return;
  }
  queueIds(ids);
  store.status = `已加入下载队列：${ids.length} 项`;
  void api.enqueue(ids);
}

export function downloadOne(id: number) {
  queueIds([id]);
  store.status = "已加入下载队列：1 项";
  void api.enqueue([id]);
}

export async function openPreview(id: number) {
  store.previewId = id;
  store.previewUrl = null;
  store.previewError = null;
  store.previewSize = { width: 0, height: 0 };
  const view = store.views[id];
  if (view?.item.kind === "Video") {
    if (view.path) {
      const [file] = await api.libraryPrepare([view.path]);
      if (store.previewId !== id) return;
      if (file?.exists) {
        store.previewUrl = api.libraryUrl(file.id);
        return;
      }
    }
    store.previewError = "视频需下载完成后才能预览";
    return;
  }
  void api.openPreview(id);
}

export function closePreview() {
  store.previewId = null;
  store.previewUrl = null;
  store.previewError = null;
}

export function toggleQueue() {
  const expanded = queueExpanded.value;
  store.queuePinned = true;
  store.queueOpen = !expanded;
}

export function openMenu(event: MouseEvent, id: number) {
  store.menu = { x: event.clientX, y: event.clientY, id };
}

export function closeMenu() {
  store.menu = null;
}

export async function copyText(text: string) {
  try {
    await navigator.clipboard.writeText(text);
  } catch {
    const area = document.createElement("textarea");
    area.value = text;
    document.body.appendChild(area);
    area.select();
    document.execCommand("copy");
    area.remove();
  }
  store.status = "已复制链接";
}

export function revealPath(path: string) {
  void api.revealPath(path);
}

export function removeHistoryPaths(paths: string[]) {
  const set = new Set(paths);
  store.history = store.history.filter((entry) => !set.has(entry.path));
}

export function openDirectory(path: string) {
  void api.openDirectory(path);
}

export function setConcurrency(value: number) {
  store.stats.concurrency = value;
  void api.setConcurrency(value);
}

export function pauseAll() {
  void api.pauseAll();
}

export function resumeAll() {
  void api.resumeAll();
}

export function retryAllFailed() {
  void api.retryAllFailed();
}

export function retryItem(id: number) {
  void api.retryItem(id);
}

export function cancelItem(id: number) {
  void api.cancelItem(id);
}

export function clearFinished() {
  store.queueIds = store.queueIds.filter((id) => {
    const view = store.views[id];
    return view && !["Completed", "Failed", "Cancelled", "Skipped"].includes(view.state);
  });
  void api.clearFinished();
}

export function removeQueueItems(ids: number[]) {
  const set = new Set(ids);
  store.queueIds = store.queueIds.filter((id) => !set.has(id));
}