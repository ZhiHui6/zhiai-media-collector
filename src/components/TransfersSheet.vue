<script setup lang="ts">
import { computed, ref, watch } from "vue";
import Icon from "./Icon.vue";
import { api } from "../api";
import {
  cancelItem,
  copyText,
  openDirectory,
  pauseAll,
  queueViews,
  removeHistoryPaths,
  removeQueueItems,
  resumeAll,
  retryItem,
  revealPath,
  store,
  totalSpeed,
} from "../store";
import {
  KIND_LABEL,
  STATE_LABEL,
  formatBytes,
  formatSpeed,
  kindTag,
  progressPercent,
  stateColor,
  type ImageKind,
  type ItemState,
} from "../types";

type Tab = "active" | "failed" | "done" | "all";

interface FileRow {
  key: string;
  id: number | null;
  source: "queue" | "history";
  title: string;
  subtitle: string;
  thumb: string | null;
  kind: ImageKind;
  state: ItemState;
  done: number;
  total: number;
  speed: number;
  message: string | null;
  path: string | null;
  order: number;
  pageUrl: string;
  pageTitle: string;
}

interface Group {
  key: string;
  title: string;
  files: FileRow[];
  order: number;
}

const tab = ref<Tab>("active");
const sort = ref<"newest" | "oldest">("newest");
const selected = ref(new Set<string>());
const expanded = ref(new Set<string>());
const libraryIds = ref<Record<string, number>>({});

const FINISHED: ItemState[] = ["Failed", "Cancelled", "Skipped"];

const queueFiles = computed<FileRow[]>(() =>
  queueViews.value
    .filter((view) => view.state !== "Completed")
    .map((view, index) => ({
      key: `q-${view.item.id}`,
      id: view.item.id,
      source: "queue" as const,
      title: `#${String(view.item.seq).padStart(3, "0")}`,
      subtitle: view.format ?? "原图",
      thumb: view.thumbUrl,
      kind: view.item.kind,
      state: view.state,
      done: view.done,
      total: view.total,
      speed: view.speed,
      message: view.message,
      path: view.path,
      order: 1e16 + index,
      pageUrl: view.item.page_url,
      pageTitle: view.item.page_title,
    })),
);

const historyFiles = computed<FileRow[]>(() =>
  store.history.map((entry, index) => ({
    key: `h-${index}-${entry.path}`,
    id: null,
    source: "history" as const,
    title: baseName(entry.path),
    subtitle: entry.format,
    thumb:
      entry.kind !== "Video" && libraryIds.value[entry.path]
        ? api.libraryUrl(libraryIds.value[entry.path])
        : null,
    kind: entry.kind,
    state: "Completed" as ItemState,
    done: entry.bytes,
    total: entry.bytes,
    speed: 0,
    message: null,
    path: entry.path,
    order: Date.parse(entry.finished_at) || 0,
    pageUrl: entry.page_url,
    pageTitle: entry.page_title,
  })),
);

const groups = computed<Group[]>(() => {
  const map = new Map<string, Group>();
  const all = [...queueFiles.value, ...historyFiles.value];
  for (const file of all) {
    const key = file.pageUrl || file.title;
    let group = map.get(key);
    if (!group) {
      group = {
        key,
        title: file.pageTitle || "未命名任务",
        files: [],
        order: 0,
      };
      map.set(key, group);
    }
    group.files.push(file);
    group.order = Math.max(group.order, file.order);
  }
  return [...map.values()];
});

function isActive(state: ItemState): boolean {
  return state === "Queued" || state === "Downloading";
}

function groupState(group: Group): ItemState {
  if (group.files.some((file) => file.state === "Downloading")) return "Downloading";
  if (group.files.some((file) => file.state === "Queued")) return "Queued";
  if (group.files.every((file) => file.state === "Completed")) return "Completed";
  if (group.files.some((file) => file.state === "Failed")) return "Failed";
  return "Cancelled";
}

function inTab(group: Group, value: Tab): boolean {
  switch (value) {
    case "active":
      return group.files.some((file) => isActive(file.state));
    case "failed":
      return group.files.some((file) => FINISHED.includes(file.state));
    case "done":
      return group.files.every((file) => file.source === "history");
    default:
      return true;
  }
}

const tabs = computed(() => [
  { id: "active" as Tab, label: "活动", count: groups.value.filter((g) => inTab(g, "active")).length },
  { id: "failed" as Tab, label: "失败", count: groups.value.filter((g) => inTab(g, "failed")).length },
  { id: "done" as Tab, label: "已完成", count: groups.value.filter((g) => inTab(g, "done")).length },
  { id: "all" as Tab, label: "全部", count: groups.value.length },
]);

const rows = computed<Group[]>(() =>
  groups.value
    .filter((group) => inTab(group, tab.value))
    .sort((a, b) => (sort.value === "newest" ? b.order - a.order : a.order - b.order)),
);

const selectedGroups = computed(() => rows.value.filter((group) => selected.value.has(group.key)));
const selectedFiles = computed(() => selectedGroups.value.flatMap((group) => group.files));
const allSelected = computed(
  () => rows.value.length > 0 && selectedGroups.value.length === rows.value.length,
);
const hasCancellable = computed(() => selectedFiles.value.some((file) => file.id != null && isActive(file.state)));
const hasRetryable = computed(() =>
  selectedFiles.value.some((file) => file.id != null && FINISHED.includes(file.state)),
);
const hasRemovable = computed(() =>
  selectedFiles.value.some((file) => file.source === "history" || FINISHED.includes(file.state)),
);

watch(
  () => store.history.map((entry) => entry.path).join("\n"),
  async () => {
    if (store.history.length === 0) {
      libraryIds.value = {};
      return;
    }
    const result = await api.libraryPrepare(store.history.map((entry) => entry.path));
    const map: Record<string, number> = {};
    for (const file of result) if (file.exists) map[file.path] = file.id;
    libraryIds.value = map;
  },
  { immediate: true },
);

function baseName(path: string): string {
  const parts = path.split(/[\\/]/);
  return parts[parts.length - 1] ?? path;
}

function labelOf(state: ItemState): string {
  if (store.stats.paused && isActive(state)) return "已暂停";
  return STATE_LABEL[state];
}

function colorOf(state: ItemState): string {
  if (store.stats.paused && isActive(state)) return "var(--orange)";
  return stateColor(state);
}

function progressOf(file: FileRow): number {
  if (file.state === "Completed") return 100;
  return progressPercent(file.done, file.total);
}

function groupProgress(group: Group): number {
  if (group.files.every((file) => file.state === "Completed")) return 100;
  const total = group.files.reduce((sum, file) => sum + file.total, 0);
  const done = group.files.reduce((sum, file) => sum + file.done, 0);
  return progressPercent(done, total);
}

function groupBytes(group: Group): number {
  return group.files.reduce((sum, file) => sum + (file.total || file.done), 0);
}

function groupSpeed(group: Group): number {
  return group.files.reduce((sum, file) => sum + (file.state === "Downloading" ? file.speed : 0), 0);
}

function toggleGroup(group: Group) {
  if (selected.value.has(group.key)) selected.value.delete(group.key);
  else selected.value.add(group.key);
}

function toggleAll() {
  if (allSelected.value) {
    for (const group of rows.value) selected.value.delete(group.key);
  } else {
    for (const group of rows.value) selected.value.add(group.key);
  }
}

function toggleExpand(group: Group) {
  if (expanded.value.has(group.key)) expanded.value.delete(group.key);
  else expanded.value.add(group.key);
}

function cancelGroup(group: Group) {
  for (const file of group.files) {
    if (file.id != null && isActive(file.state)) cancelItem(file.id);
  }
}

function retryGroup(group: Group) {
  for (const file of group.files) {
    if (file.id != null && FINISHED.includes(file.state)) retryItem(file.id);
  }
}

function cancelSelected() {
  for (const file of selectedFiles.value) {
    if (file.id != null && isActive(file.state)) cancelItem(file.id);
  }
}

function resumeSelected() {
  if (store.stats.paused) resumeAll();
  for (const file of selectedFiles.value) {
    if (file.id != null && FINISHED.includes(file.state)) retryItem(file.id);
  }
}

async function removeSelected() {
  const files = selectedFiles.value;
  const ids = files.filter((file) => file.id != null && FINISHED.includes(file.state)).map((file) => file.id!);
  const paths = files.filter((file) => file.source === "history" && file.path).map((file) => file.path!);
  if (ids.length) removeQueueItems(ids);
  if (paths.length) {
    await api.libraryDelete(paths, false);
    removeHistoryPaths(paths);
  }
  for (const group of selectedGroups.value) selected.value.delete(group.key);
  store.status = `已移除 ${ids.length + paths.length} 个文件`;
}
</script>

<template>
  <section class="page">
    <header class="tf-top">
      <h1>传输</h1>
      <span class="spacer"></span>
      <button
        class="ibtn sm"
        v-tip="store.stats.paused ? '继续全部' : '暂停全部'"
        @click="store.stats.paused ? resumeAll() : pauseAll()"
      >
        <Icon :name="store.stats.paused ? 'play' : 'pause'" :size="14" />
      </button>
      <button class="ibtn sm" v-tip="'打开下载目录'" @click="openDirectory(store.settings.download_root)">
        <Icon name="folder" :size="14" />
      </button>
      <select v-model="sort" class="field tf-sort" aria-label="排序方式">
        <option value="newest">最新添加在前</option>
        <option value="oldest">最早添加在前</option>
      </select>
    </header>

    <div class="tf-bar">
      <button
        v-for="item in tabs"
        :key="item.id"
        class="tf-tab"
        :aria-selected="tab === item.id"
        @click="tab = item.id"
      >
        {{ item.label }}<span class="tf-count">{{ item.count }}</span>
      </button>
      <span class="spacer"></span>
      <span v-if="totalSpeed > 1" class="b-qmeta">{{ formatSpeed(totalSpeed) }}</span>
    </div>

    <div class="tf-table">
      <div class="tf-thead">
        <button
          class="tf-chk"
          role="checkbox"
          :aria-checked="allSelected"
          aria-label="全选"
          @click="toggleAll"
        ></button>
        <span>任务 / 文件</span>
        <span>进度</span>
        <span>状态</span>
        <span>速度</span>
        <span class="tf-ops-head">操作</span>
      </div>

      <div class="tf-rows">
        <div v-if="rows.length === 0" class="tf-empty">
          {{ tab === "active" ? "当前没有进行中的任务" : tab === "failed" ? "没有失败的任务" : tab === "done" ? "还没有已完成的下载" : "还没有传输任务" }}
        </div>

        <div v-for="group in rows" :key="group.key" class="tf-group">
          <div class="tf-trow tf-ghead" :class="{ sel: selected.has(group.key) }">
            <button
              class="tf-chk"
              role="checkbox"
              :aria-checked="selected.has(group.key)"
              aria-label="选择任务"
              @click="toggleGroup(group)"
            ></button>

            <div class="tf-file" @click="toggleExpand(group)">
              <button
                class="tf-caret"
                :class="{ open: expanded.has(group.key) }"
                :aria-label="expanded.has(group.key) ? '折叠' : '展开'"
                @click.stop="toggleExpand(group)"
              >
                <Icon name="chevron" :size="12" />
              </button>
              <span class="tf-thumb">
                <img v-if="group.files[0]?.thumb" :src="group.files[0].thumb" alt="" loading="lazy" />
                <Icon
                  v-else
                  :name="
                    group.files[0]?.kind === 'Video'
                      ? 'video'
                      : groupState(group) === 'Completed'
                        ? 'down'
                        : 'image'
                  "
                  :size="15"
                />
              </span>
              <div class="tf-file-text">
                <div class="tf-name">
                  <span class="ellipsis" v-tip="group.title">{{ group.title }}</span>
                </div>
                <div class="tf-desc">
                  {{ group.files.length }} 个文件 · {{ formatBytes(groupBytes(group)) }}
                </div>
              </div>
            </div>

            <div class="tf-progress">
              <div class="tf-prow">
                <span class="tf-pbar">
                  <i
                    :style="{ width: groupProgress(group) + '%', background: colorOf(groupState(group)) }"
                  ></i>
                </span>
                <span class="tf-pct">{{ groupProgress(group) }}%</span>
              </div>
              <div class="tf-psize">
                {{ group.files.length }} 个文件中已完成
                {{ group.files.filter((file) => file.state === "Completed").length }} 个
              </div>
            </div>

            <span class="tf-state" :style="{ color: colorOf(groupState(group)) }">
              {{ labelOf(groupState(group)) }}
            </span>

            <span class="tf-speed">
              {{ groupSpeed(group) > 1 ? formatSpeed(groupSpeed(group)) : "—" }}
            </span>

            <div class="tf-ops">
              <button
                v-if="group.files.some((file) => file.id != null && isActive(file.state))"
                class="ibtn sm"
                v-tip="'取消任务'"
                @click="cancelGroup(group)"
              >
                <Icon name="clear" :size="13" />
              </button>
              <button
                v-else-if="group.files.some((file) => file.id != null && FINISHED.includes(file.state))"
                class="ibtn sm"
                v-tip="'重试失败文件'"
                @click="retryGroup(group)"
              >
                <Icon name="retry" :size="13" />
              </button>
              <button
                v-else
                class="ibtn sm"
                v-tip="'在文件夹中显示'"
                @click="revealPath(group.files[0]?.path ?? '')"
              >
                <Icon name="folder" :size="13" />
              </button>
            </div>
          </div>

          <template v-if="expanded.has(group.key)">
            <div v-for="file in group.files" :key="file.key" class="tf-trow tf-childrow">
              <span></span>

              <div class="tf-file">
                <span class="tf-thumb sm">
                  <img v-if="file.thumb" :src="file.thumb" alt="" loading="lazy" />
                  <Icon
                    v-else
                    :name="
                      file.kind === 'Video'
                        ? 'video'
                        : file.state === 'Completed'
                          ? 'down'
                          : 'image'
                    "
                    :size="13"
                  />
                </span>
                <div class="tf-file-text">
                  <div class="tf-name">
                    <span class="ellipsis" v-tip="file.title">{{ file.title }}</span>
                    <span class="tag" :class="kindTag(file.kind)">{{ KIND_LABEL[file.kind] }}</span>
                  </div>
                  <div class="tf-desc ellipsis">
                    {{ file.state === "Failed" && file.message ? file.message : file.subtitle }}
                  </div>
                </div>
              </div>

              <div class="tf-progress">
                <div class="tf-prow">
                  <span class="tf-pbar">
                    <i
                      :style="{ width: progressOf(file) + '%', background: colorOf(file.state) }"
                    ></i>
                  </span>
                  <span class="tf-pct">{{ progressOf(file) }}%</span>
                </div>
                <div class="tf-psize">
                  {{ file.total > 0 ? `${formatBytes(file.done)} / ${formatBytes(file.total)}` : "—" }}
                </div>
              </div>

              <span class="tf-state" :style="{ color: colorOf(file.state) }">
                {{ labelOf(file.state) }}
              </span>

              <span class="tf-speed">
                {{ file.state === "Downloading" && file.speed > 1 ? formatSpeed(file.speed) : "—" }}
              </span>

              <div class="tf-ops">
                <template v-if="file.id != null">
                  <button
                    v-if="isActive(file.state)"
                    class="ibtn sm"
                    v-tip="'取消'"
                    @click="cancelItem(file.id)"
                  >
                    <Icon name="clear" :size="13" />
                  </button>
                  <button
                    v-else-if="FINISHED.includes(file.state)"
                    class="ibtn sm"
                    v-tip="'重试'"
                    @click="retryItem(file.id)"
                  >
                    <Icon name="retry" :size="13" />
                  </button>
                </template>
                <template v-else>
                  <button class="ibtn sm" v-tip="'在文件夹中显示'" @click="revealPath(file.path ?? '')">
                    <Icon name="folder" :size="13" />
                  </button>
                  <button class="ibtn sm" v-tip="'复制文件路径'" @click="copyText(file.path ?? '')">
                    <Icon name="copy" :size="13" />
                  </button>
                </template>
              </div>
            </div>
          </template>
        </div>
      </div>
    </div>

    <footer class="tf-footer">
      <span class="hint">
        已选择 {{ selectedGroups.length }} 个任务 · {{ selectedFiles.length }} 个文件
      </span>
      <span class="spacer"></span>
      <button class="btn sm" :disabled="!hasCancellable" @click="cancelSelected">取消</button>
      <button class="btn sm" :disabled="!hasRetryable && !store.stats.paused" @click="resumeSelected">
        继续
      </button>
      <button class="btn sm" :disabled="!hasRemovable" @click="removeSelected">移除任务</button>
    </footer>
  </section>
</template>
