<script setup lang="ts">
import { computed } from "vue";
import Icon from "./Icon.vue";
import { openMenu, openPreview, toggleSelect, type View } from "../store";
import {
  KIND_LABEL,
  STATE_LABEL,
  formatBytes,
  kindTag,
  progressPercent,
  stateColor,
} from "../types";

const props = defineProps<{ view: View }>();

const percent = computed(() => progressPercent(props.view.done, props.view.total));

const meta = computed(() => {
  const view = props.view;
  if (view.state === "Downloading") return "";
  if (view.state === "Completed") {
    const size = view.bytes != null ? formatBytes(view.bytes) : "";
    return [view.format ?? "已完成", size].filter(Boolean).join(" · ");
  }
  if (view.state === "Failed") return "下载失败";
  return "未下载";
});
</script>

<template>
  <article
    class="b-card"
    :class="{ selected: view.item.selected }"
    @click="toggleSelect(view.item.id)"
    @dblclick="openPreview(view.item.id)"
    @contextmenu.prevent.stop="openMenu($event, view.item.id)"
  >
    <div class="b-thumb">
      <img v-if="view.thumbUrl" :src="view.thumbUrl" alt="" loading="lazy" />
      <div v-else-if="view.item.kind === 'Video'" class="b-thumb-video">
        <Icon name="video" :size="22" />
      </div>
      <div v-else class="skeleton"></div>
      <span class="b-seq">{{ String(view.item.seq).padStart(3, "0") }}</span>
      <span class="b-check" :class="{ on: view.item.selected }"></span>
    </div>
    <div class="b-meta">
      <span class="tag" :class="kindTag(view.item.kind)">{{ KIND_LABEL[view.item.kind] }}</span>
      <span class="b-state">
        <i class="dot" :style="{ background: stateColor(view.state) }"></i>
        {{ STATE_LABEL[view.state] }}
      </span>
      <span class="spacer"></span>
      <span v-if="view.state === 'Downloading'" class="b-pct">{{ percent }}%</span>
      <span v-else class="b-size">{{ meta }}</span>
    </div>
    <div v-if="view.state === 'Downloading'" class="b-progress">
      <i :style="{ width: percent + '%' }"></i>
    </div>
  </article>
</template>