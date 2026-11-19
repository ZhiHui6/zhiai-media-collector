<script setup lang="ts">
import Icon from "./Icon.vue";
import { store } from "../store";

const links = [
  { icon: "code", title: "项目仓库", url: "待补充" },
  { icon: "info", title: "操作指南", url: "待补充" },
  { icon: "mail", title: "反馈建议", url: "待补充" },
];
</script>

<template>
  <section class="page">
    <header class="wb-top">
      <h1>关于</h1>
      <span class="spacer"></span>
    </header>

    <div class="page-body">
      <div class="about-hero">
        <span class="about-mark">智爱</span>
        <span class="about-id">
          <b>智爱媒体采集器</b>
          <small>Zhiai Media Collector</small>
        </span>
      </div>

      <p class="about-desc">解析、筛选和下载微信公众号文章里的图片素材。</p>

      <div class="about-ver">
        <span class="muted">当前版本</span>
        <span class="mono">v{{ store.version }}</span>
      </div>

      <div class="about-links">
        <div v-for="link in links" :key="link.title" class="about-link">
          <span class="al-icon"><Icon :name="link.icon" :size="18" /></span>
          <span class="al-text">
            <b>{{ link.title }}</b>
            <small>{{ link.url }}</small>
          </span>
          <span class="al-out"><Icon name="external" :size="14" /></span>
        </div>
      </div>
    </div>
  </section>
</template>
