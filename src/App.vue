<script setup lang="ts">
import { RouterView } from "vue-router";
</script>

<template>
  <router-view />
</template>

<style>
/* 根元素全局样式：原子类无法作用于 html/body，此处保留必要的全局规则 */
html,
body,
#app {
  margin: 0;
  height: 100%;
}
#app {
  font-family:
    -apple-system, BlinkMacSystemFont, "Segoe UI", "PingFang SC",
    "Microsoft YaHei", sans-serif;
}
</style>
