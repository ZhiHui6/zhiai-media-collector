<script setup lang="ts">
import { computed, onBeforeUnmount, reactive, ref, watch } from "vue";
import { open } from "@tauri-apps/plugin-dialog";
import Icon from "./Icon.vue";
import { api, describeError, type DirectoryStatus } from "../api";
import {
  openDirectory,
  restoreDefaultSettings,
  saveSettingsSoon,
  setTheme,
  store,
} from "../store";
import type { ConvertTarget, DuplicatePolicy, ThemeMode } from "../types";

const themes: Array<{ value: ThemeMode; label: string }> = [
  { value: "System", label: "跟随系统" },
  { value: "Light", label: "亮色" },
  { value: "Dark", label: "暗色" },
];

const targets: Array<{ value: ConvertTarget; label: string }> = [
  { value: "Jpg", label: "JPG" },
  { value: "Png", label: "PNG" },
  { value: "WebP", label: "WebP" },
];

const duplicatePolicies: Array<{ value: DuplicatePolicy; label: string }> = [
  { value: "AppendSuffix", label: "自动加序号" },
  { value: "Skip", label: "跳过已有文件" },
  { value: "Overwrite", label: "覆盖已有文件" },
];

type NumberKey =
  | "concurrency"
  | "retry_max"
  | "jpeg_quality"
  | "webp_quality"
  | "speed_limit_kbps"
  | "parse_interval_ms";

const errors = reactive<Record<string, string>>({});

function applyNumber(key: NumberKey, label: string, min: number, max: number) {
  const value = Number(store.settings[key]);
  if (!Number.isFinite(value) || value < min || value > max) {
    errors[key] = `${label}需在 ${min}–${max} 之间`;
    return;
  }
  errors[key] = "";
  store.settings[key] = Math.round(value);
  saveSettingsSoon();
}

const draft = ref(store.settings.download_root);
const dirError = ref("");

watch(
  () => store.settings.download_root,
  (value) => {
    draft.value = value;
  },
);

const dirty = computed(() => draft.value.trim() !== store.settings.download_root);

const effective = computed(() => {
  const root = store.settings.download_root.replace(/[\\/]+$/, "");
  return root ? `${root}\\<文章标题>\\` : "";
});

const health = ref<DirectoryStatus | null>(null);
let healthTimer: number | undefined;

async function checkHealth() {
  const root = store.settings.download_root;
  if (!root) {
    health.value = null;
    return;
  }
  try {
    health.value = await api.checkDirectory(root);
  } catch {
    health.value = null;
  }
}

watch(
  () => store.settings.download_root,
  () => {
    window.clearTimeout(healthTimer);
    healthTimer = window.setTimeout(() => void checkHealth(), 300);
  },
  { immediate: true },
);

onBeforeUnmount(() => window.clearTimeout(healthTimer));

const healthText = computed(() => {
  if (!health.value) return "";
  if (!health.value.exists) return "目录不存在，应用保存目录时会自动创建";
  return health.value.writable ? "目录可写" : "目录不可写，请检查权限或更换目录";
});

const healthError = computed(() => Boolean(health.value && health.value.exists && !health.value.writable));

function toggleConvert() {
  store.settings.convert_enabled = !store.settings.convert_enabled;
  saveSettingsSoon();
}

async function applyPath() {
  dirError.value = "";
  if (!draft.value.trim()) {
    dirError.value = "目录不能为空";
    return;
  }
  try {
    const applied = await api.setDownloadRoot(draft.value);
    store.settings.download_root = applied;
    draft.value = applied;
  } catch (error) {
    dirError.value = describeError(error);
  }
}

async function browse() {
  dirError.value = "";
  try {
    const selected = await open({
      directory: true,
      multiple: false,
      title: "选择下载目录",
      defaultPath: store.settings.download_root || undefined,
    });
    if (typeof selected === "string") {
      draft.value = selected;
      await applyPath();
    }
  } catch (error) {
    dirError.value = `无法打开目录选择器：${describeError(error)}`;
  }
}
</script>

<template>
  <section class="page">
    <header class="wb-top">
      <h1>设置</h1>
      <span class="spacer"></span>
      <button class="btn sm" @click="restoreDefaultSettings()">恢复默认</button>
    </header>

    <div class="page-body">
        <p
          v-if="store.settingsNotice"
          class="settings-notice"
          :class="{ danger: store.settingsNotice.error }"
        >
          {{ store.settingsNotice.text }}
        </p>

        <p class="group">外观</p>
        <div class="cells">
          <div class="cell">
            <span>主题</span>
            <span class="spacer"></span>
            <div class="seg">
              <button
                v-for="option in themes"
                :key="option.value"
                :aria-pressed="store.settings.theme === option.value"
                @click="setTheme(option.value)"
              >
                {{ option.label }}
              </button>
            </div>
          </div>
        </div>

        <p class="group">下载</p>
        <div class="cells">
          <div class="cell">
            <span>保存目录</span>
            <span class="spacer"></span>
            <button class="btn sm" @click="browse">浏览…</button>
            <button class="btn sm" @click="openDirectory(store.settings.download_root)">
              <Icon name="folder" :size="14" />打开
            </button>
          </div>
          <div class="dir-row">
            <input
              class="field grow"
              v-model="draft"
              aria-label="保存目录"
              placeholder="输入或粘贴任意绝对路径"
              @keydown.enter="applyPath"
            />
            <button class="btn sm" :disabled="!dirty" @click="applyPath">应用</button>
          </div>
          <p class="cell-note" :class="{ danger: dirError }" v-tip="dirError || effective">
            {{ dirError || `实际保存到 ${effective}` }}
          </p>
          <p class="cell-note" :class="{ danger: healthError }">
            {{ healthText || "正在检查保存目录…" }}
          </p>
          <div class="cell">
            <span>并发数</span>
            <span class="spacer"></span>
            <span class="hint">1–16</span>
            <input
              class="field num"
              type="number"
              min="1"
              max="16"
              v-model.number="store.settings.concurrency"
              aria-label="并发数"
              :aria-invalid="Boolean(errors.concurrency)"
              @change="applyNumber('concurrency', '并发数', 1, 16)"
            />
          </div>
          <p v-if="errors.concurrency" class="cell-note danger">{{ errors.concurrency }}</p>
          <div class="cell">
            <span>重试次数</span>
            <span class="spacer"></span>
            <span class="hint">0–5</span>
            <input
              class="field num"
              type="number"
              min="0"
              max="5"
              v-model.number="store.settings.retry_max"
              aria-label="重试次数"
              :aria-invalid="Boolean(errors.retry_max)"
              @change="applyNumber('retry_max', '重试次数', 0, 5)"
            />
          </div>
          <p v-if="errors.retry_max" class="cell-note danger">{{ errors.retry_max }}</p>
          <div class="cell">
            <span>同名文件</span>
            <span class="spacer"></span>
            <select
              class="field"
              :value="store.settings.duplicate_policy"
              aria-label="同名文件处理"
              @change="
                store.settings.duplicate_policy = ($event.target as HTMLSelectElement)
                  .value as DuplicatePolicy;
                saveSettingsSoon()
              "
            >
              <option v-for="option in duplicatePolicies" :key="option.value" :value="option.value">
                {{ option.label }}
              </option>
            </select>
          </div>
          <p class="cell-note">重复采集同一链接时：跳过、自动加序号或覆盖已有文件</p>
          <div class="cell">
            <span>下载限速</span>
            <span class="spacer"></span>
            <span class="hint">KB/s，0 不限</span>
            <input
              class="field num"
              type="number"
              min="0"
              max="1048576"
              v-model.number="store.settings.speed_limit_kbps"
              aria-label="下载限速"
              :aria-invalid="Boolean(errors.speed_limit_kbps)"
              @change="applyNumber('speed_limit_kbps', '限速', 0, 1048576)"
            />
          </div>
          <p v-if="errors.speed_limit_kbps" class="cell-note danger">
            {{ errors.speed_limit_kbps }}
          </p>
          <div class="cell">
            <span>解析间隔</span>
            <span class="spacer"></span>
            <span class="hint">毫秒，0 不等待</span>
            <input
              class="field num"
              type="number"
              min="0"
              max="10000"
              v-model.number="store.settings.parse_interval_ms"
              aria-label="解析间隔"
              :aria-invalid="Boolean(errors.parse_interval_ms)"
              @change="applyNumber('parse_interval_ms', '解析间隔', 0, 10000)"
            />
          </div>
          <p v-if="errors.parse_interval_ms" class="cell-note danger">
            {{ errors.parse_interval_ms }}
          </p>
          <p class="cell-note">重试仅针对网络错误与 5xx/429 响应；解析间隔用于批量解析限速</p>
        </div>

        <p class="group">格式转换</p>
        <div class="cells">
          <div class="cell">
            <span>下载后转换格式</span>
            <span class="spacer"></span>
            <button
              class="switch"
              role="switch"
              :aria-checked="store.settings.convert_enabled"
              aria-label="下载后转换格式"
              @click="toggleConvert"
            ></button>
          </div>
          <div class="cell" :class="{ disabled: !store.settings.convert_enabled }">
            <span>目标格式</span>
            <span class="spacer"></span>
            <select
              class="field"
              :value="store.settings.convert_target"
              :disabled="!store.settings.convert_enabled"
              aria-label="目标格式"
              @change="store.settings.convert_target = ($event.target as HTMLSelectElement).value as ConvertTarget; saveSettingsSoon()"
            >
              <option v-for="option in targets" :key="option.value" :value="option.value">
                {{ option.label }}
              </option>
            </select>
          </div>
          <div :class="{ disabled: !store.settings.convert_enabled }">
            <div class="srow">
              <div class="srow-top">
                <span>JPEG 质量</span>
                <span class="spacer"></span>
                <span class="hint">范围 1–100 %</span>
              </div>
              <div class="srow-ctl">
                <input
                  type="range"
                  min="1"
                  max="100"
                  v-model.number="store.settings.jpeg_quality"
                  :disabled="!store.settings.convert_enabled"
                  aria-label="JPEG 质量"
                  @input="saveSettingsSoon"
                />
                <input
                  class="field num"
                  type="number"
                  min="1"
                  max="100"
                  v-model.number="store.settings.jpeg_quality"
                  :disabled="!store.settings.convert_enabled"
                  aria-label="JPEG 质量数值"
                  :aria-invalid="Boolean(errors.jpeg_quality)"
                  @change="applyNumber('jpeg_quality', 'JPEG 质量', 1, 100)"
                />
                <span class="hint">%</span>
              </div>
              <p v-if="errors.jpeg_quality" class="cell-note danger">{{ errors.jpeg_quality }}</p>
            </div>
            <div class="srow">
              <div class="srow-top">
                <span>WebP 质量</span>
                <span class="spacer"></span>
                <span class="hint">范围 1–100 %</span>
              </div>
              <div class="srow-ctl">
                <input
                  type="range"
                  min="1"
                  max="100"
                  v-model.number="store.settings.webp_quality"
                  :disabled="!store.settings.convert_enabled"
                  aria-label="WebP 质量"
                  @input="saveSettingsSoon"
                />
                <input
                  class="field num"
                  type="number"
                  min="1"
                  max="100"
                  v-model.number="store.settings.webp_quality"
                  :disabled="!store.settings.convert_enabled"
                  aria-label="WebP 质量数值"
                  :aria-invalid="Boolean(errors.webp_quality)"
                  @change="applyNumber('webp_quality', 'WebP 质量', 1, 100)"
                />
                <span class="hint">%</span>
              </div>
              <p v-if="errors.webp_quality" class="cell-note danger">{{ errors.webp_quality }}</p>
            </div>
          </div>
          <p class="cell-note">GIF 与动画 WebP 始终保留原始字节</p>
        </div>

        <p class="group">数据</p>
        <div class="cells">
          <div class="cell">
            <span>已记录下载</span>
            <span class="spacer"></span>
            <span class="hint">{{ store.history.length }} 条</span>
          </div>
          <div class="cell">
            <span>单图上限</span>
            <span class="spacer"></span>
            <span class="hint">64 MB</span>
          </div>
          <div class="cell">
            <button class="btn sm" @click="openDirectory(store.settings.download_root)">
              <Icon name="folder" :size="14" />打开下载目录
            </button>
          </div>
        </div>
      </div>
  </section>
</template>