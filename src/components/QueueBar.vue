<script setup lang="ts">
import { computed } from "vue";
import Icon from "./Icon.vue";
import {
  cancelItem,
  clearFinished,
  downloadSelected,
  invertSelection,
  pauseAll,
  queueExpanded,
  queueViews,
  resumeAll,
  retryAllFailed,
  retryItem,
  selectAll,
  setConcurrency,
  store,
  toggleQueue,
  totalSpeed,
} from "../store";
import {
  KIND_LABEL,
  STATE_LABEL,
  formatSpeed,
  kindTag,
  progressPercent,
  stateColor,
} from "../types";

const current = computed(
  () => queueViews.value.find((view) => view.state === "Downloading") ?? queueViews.value[0],
);

const percent = computed(() => {
  const view = current.value;
  return view ? progressPercent(view.done, view.total) : 0;
});

const statusWord = computed(() => {
  const stats = store.stats;
  if (stats.paused) return { color: "var(--orange)", label: "已暂停" };
  if (stats.active > 0) return { color: "var(--blue)", label: "下载中" };
  if (stats.failed > 0) return { color: "var(--red)", label: "有失败" };
  if (stats.queued > 0) return { color: "var(--faint)", label: "等待中" };
  return { color: "var(--brand)", label: "空闲" };
});

function onConcurrency(event: Event) {
  const value = Number((event.target as HTMLInputElement).value);
  if (Number.isFinite(value)) {
    setConcurrency(Math.min(16, Math.max(1, Math.round(value))));
  }
}
</script>

<template>
  <section class="b-dock">
    <div v-if="queueExpanded && queueViews.length" class="b-qlist">
      <div v-for="view in queueViews" :key="view.item.id" class="b-qrow">
        <span class="b-state">
          <i class="dot" :style="{ background: stateColor(view.state) }"></i>
          {{ STATE_LABEL[view.state] }}
        </span>
        <span class="b-qtitle">
          #{{ String(view.item.seq).padStart(3, "0") }} {{ view.item.page_title }}
        </span>
        <span class="tag" :class="kindTag(view.item.kind)">{{ KIND_LABEL[view.item.kind] }}</span>
        <template v-if="view.state === 'Downloading'">
          <span class="b-qbar"><i :style="{ width: progressPercent(view.done, view.total) + '%' }"></i></span>
          <span class="b-qpct">{{ progressPercent(view.done, view.total) }}%</span>
          <span v-if="view.speed > 1" class="b-qmeta">{{ formatSpeed(view.speed) }}</span>
        </template>
        <span v-else-if="view.state === 'Failed'" class="b-qmeta danger" v-tip="view.message ?? '下载失败'">
          {{ view.action ? `建议：${view.action}` : view.message ?? "下载失败" }}
        </span>
        <span v-else-if="view.state === 'Skipped'" class="b-qmeta">{{ view.message ?? "已跳过" }}</span>
        <span class="spacer"></span>
        <button v-if="view.state === 'Downloading' || view.state === 'Queued'" class="btn sm" @click="cancelItem(view.item.id)">取消</button>
        <button v-else-if="view.state === 'Failed' || view.state === 'Cancelled'" class="btn sm" @click="retryItem(view.item.id)">重试</button>
      </div>
    </div>

    <div v-if="queueViews.length" class="b-strip">
      <span class="b-state">
        <i class="dot" :style="{ background: statusWord.color }"></i>
        {{ statusWord.label }}
      </span>
      <span v-if="current" class="b-strip-title">
        #{{ String(current.item.seq).padStart(3, "0") }} · {{ percent }}%
      </span>
      <span class="b-strip-bar"><i :style="{ width: percent + '%' }"></i></span>
      <span v-if="totalSpeed > 1" class="b-qmeta">{{ formatSpeed(totalSpeed) }}</span>
      <span class="spacer"></span>
      <span class="b-qmeta">
        排队 {{ store.stats.queued }} · 进行 {{ store.stats.active }} · 完成
        {{ store.stats.completed }} · 失败 {{ store.stats.failed }}
      </span>
      <button class="ibtn sm" v-tip="queueExpanded ? '收起队列' : '展开队列'" @click="toggleQueue">
        <Icon name="chevron" :size="12" />
      </button>
    </div>

    <div class="b-cmdbar">
      <span class="b-state">
        <i class="dot" :style="{ background: statusWord.color }"></i>
        {{ statusWord.label }}
      </span>
      <span class="b-qmeta">已选 {{ store.order.filter((id) => store.views[id]?.item.selected).length }} 项</span>
      <span class="spacer"></span>
      <label class="b-conc">
        <span class="b-qmeta">并发</span>
        <input class="field num" type="number" min="1" max="16" :value="store.stats.concurrency" @change="onConcurrency" />
      </label>
      <button class="btn sm" v-tip="'全选当前筛选'" @click="selectAll(true)">全选</button>
      <button class="btn sm" v-tip="'反选当前筛选'" @click="invertSelection">反选</button>
      <button class="btn sm" @click="store.stats.paused ? resumeAll() : pauseAll()">
        <Icon :name="store.stats.paused ? 'play' : 'pause'" :size="14" />
        {{ store.stats.paused ? "继续" : "暂停" }}
      </button>
      <button class="btn sm" :disabled="store.stats.failed === 0" @click="retryAllFailed">
        <Icon name="retry" :size="14" />重试失败
      </button>
      <button class="btn sm" @click="clearFinished"><Icon name="trash" :size="14" />清除完成</button>
      <button class="btn sm pri" @click="downloadSelected">
        <Icon name="down" />下载所选
      </button>
    </div>
  </section>
</template>