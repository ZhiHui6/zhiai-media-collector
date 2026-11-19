<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { open } from "@tauri-apps/plugin-dialog";
import Icon from "./Icon.vue";
import { api } from "../api";
import {
  openDirectory,
  saveSettingsSoon,
  setTheme,
  store,
} from "../store";
import type { ConvertTarget, ThemeMode } from "../types";

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
    dirError.value = typeof error === "string" ? error : String(error);
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
    dirError.value = `无法打开目录选择器：${String(error)}`;
  }
}
</script>

<template>
  <section class="page">
    <header class="wb-top">
      <h1>设置</h1>
      <span class="spacer"></span>
    </header>

    <div class="page-body">
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
              @change="saveSettingsSoon"
            />
          </div>
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
              @change="saveSettingsSoon"
            />
          </div>
          <p class="cell-note">重试仅针对网络错误与 5xx/429 响应</p>
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
                  @change="saveSettingsSoon"
                />
                <span class="hint">%</span>
              </div>
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
                  @change="saveSettingsSoon"
                />
                <span class="hint">%</span>
              </div>
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