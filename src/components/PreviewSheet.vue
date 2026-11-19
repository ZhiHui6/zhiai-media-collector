<script setup lang="ts">
import { computed } from "vue";
import Icon from "./Icon.vue";
import { closePreview, copyText, downloadOne, revealPath, store } from "../store";
import {
  KIND_LABEL,
  STATE_LABEL,
  formatBytes,
  kindTag,
  stateColor,
  type ImageKind,
  type ItemState,
} from "../types";

const view = computed(() => (store.previewId != null ? store.views[store.previewId] : undefined));
const itemId = computed(() => view.value?.item.id ?? 0);
const kind = computed<ImageKind>(() => view.value?.item.kind ?? "Unknown");
const state = computed<ItemState>(() => view.value?.state ?? "Queued");
const seq = computed(() => view.value?.item.seq ?? 0);
const url = computed(() => view.value?.item.url ?? "");
const path = computed(() => view.value?.path ?? null);
const format = computed(() => view.value?.format ?? null);
const bytes = computed(() => view.value?.bytes ?? null);
</script>

<template>
  <div class="scrim preview" @click.self="closePreview">
    <div v-if="view" class="preview-box" role="dialog" aria-label="图片预览">
      <header class="preview-head">
        <span class="tag" :class="kindTag(kind)">{{ KIND_LABEL[kind] }}</span>
        <span class="b-state">
          <i class="dot" :style="{ background: stateColor(state) }"></i>
          {{ STATE_LABEL[state] }}
        </span>
        <span class="hint mono">{{ String(seq).padStart(3, "0") }}</span>
        <span v-if="format && bytes != null" class="hint">
          {{ format }} · {{ formatBytes(bytes) }}
        </span>
        <span class="spacer"></span>
        <button class="ibtn" v-tip="'关闭 (Esc)'" @click="closePreview">
          <Icon name="clear" />
        </button>
      </header>

      <div class="preview-stage">
        <video
          v-if="store.previewUrl && kind === 'Video'"
          class="preview-video"
          :src="store.previewUrl"
          controls
          autoplay
        ></video>
        <img v-else-if="store.previewUrl" :src="store.previewUrl" alt="" />
        <div v-else-if="store.previewError" class="preview-error">{{ store.previewError }}</div>
        <div v-else class="preview-loading">
          <span class="spinner"></span>
          {{ kind === "Video" ? "正在加载视频…" : "正在加载原图…" }}
        </div>
      </div>

      <p class="preview-url ellipsis" v-tip="url">{{ url }}</p>

      <footer class="preview-actions">
        <button v-if="state !== 'Completed'" class="btn pri" @click="downloadOne(itemId)">
          <Icon name="down" />{{ kind === "Video" ? "下载视频" : "下载此张" }}
        </button>
        <button class="btn" @click="copyText(url)"><Icon name="copy" />复制链接</button>
        <button class="btn" :disabled="!path" @click="path && revealPath(path)">
          <Icon name="folder" />打开目录
        </button>
      </footer>
    </div>
  </div>
</template>