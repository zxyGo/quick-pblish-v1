<script setup lang="ts">
// 002-multi-platform-publish：GitHub 图床配置（repo/branch/token/useCDN）。
// 配置存浏览器 localStorage，由 services/imageHost.ts 在同步前上传本地图片换外链时读取。
import { ref, watch } from "vue";
import { MessagePlugin } from "tdesign-vue-next";
import { getGithubConfig, saveGithubConfig } from "@/services/imageHost";

const props = defineProps<{ visible: boolean }>();
const emit = defineEmits<{ "update:visible": [value: boolean] }>();

const repo = ref("");
const branch = ref("main");
const accessToken = ref("");
const useCDN = ref(false);

// 打开时回填已保存配置
watch(
  () => props.visible,
  (v) => {
    if (!v) return;
    const cfg = getGithubConfig();
    if (cfg) {
      repo.value = cfg.repo;
      branch.value = cfg.branch;
      accessToken.value = cfg.accessToken;
      useCDN.value = cfg.useCDN;
    }
  },
);

function save() {
  if (!repo.value.trim() || !accessToken.value.trim()) {
    MessagePlugin.warning("请填写仓库与 Access Token");
    return;
  }
  saveGithubConfig({
    repo: repo.value.trim(),
    branch: branch.value.trim() || "main",
    accessToken: accessToken.value.trim(),
    useCDN: useCDN.value,
  });
  MessagePlugin.success("图床配置已保存");
  emit("update:visible", false);
}
</script>

<template>
  <t-dialog
    :visible="props.visible"
    header="GitHub 图床设置"
    :footer="false"
    width="480px"
    @close="emit('update:visible', false)"
  >
    <div class="flex flex-col gap-3">
      <p class="text-sm text-gray-500">
        正文中的本地图片会在同步前上传到此仓库换成公网外链，再由各平台粘贴时自动转存。
      </p>
      <div class="flex flex-col gap-1">
        <span class="text-sm font-medium">仓库</span>
        <t-input v-model="repo" placeholder="username/repo（或完整仓库 URL）" />
      </div>
      <div class="flex flex-col gap-1">
        <span class="text-sm font-medium">分支</span>
        <t-input v-model="branch" placeholder="main" />
      </div>
      <div class="flex flex-col gap-1">
        <span class="text-sm font-medium">Access Token</span>
        <t-input
          v-model="accessToken"
          type="password"
          placeholder="GitHub Personal Access Token（需 contents 写权限）"
        />
      </div>
      <t-checkbox v-model="useCDN">使用 jsDelivr CDN 加速外链</t-checkbox>
      <div class="flex justify-end">
        <t-button @click="save">保存</t-button>
      </div>
    </div>
  </t-dialog>
</template>
