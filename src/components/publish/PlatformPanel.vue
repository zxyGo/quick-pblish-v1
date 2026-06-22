<script setup lang="ts">
// 002-multi-platform-publish US1：平台连接面板（FR-001/003/004/006）。
import { onMounted, ref } from "vue";
import { MessagePlugin } from "tdesign-vue-next";
import { usePublishStore } from "@/stores/publish";
import { PLATFORM_LABELS, type PlatformId, type PlatformStatus } from "@/bindings/types";
import { toAppError } from "@/services/error";

const store = usePublishStore();

onMounted(() => {
  store.refreshPlatforms().catch((e) => MessagePlugin.error(toAppError(e).message));
});

const STATUS_THEME: Record<PlatformStatus, "success" | "default" | "warning"> = {
  Connected: "success",
  Disconnected: "default",
  NeedReauth: "warning",
};
const STATUS_TEXT: Record<PlatformStatus, string> = {
  Connected: "已连接",
  Disconnected: "未连接",
  NeedReauth: "需重新登录",
};

// 已点过“登录”、正等待用户完成登录的平台（用于展示“我已登录”按钮）
const pending = ref<Set<PlatformId>>(new Set());

async function connect(platform: PlatformId) {
  try {
    await store.connect(platform);
    pending.value = new Set(pending.value).add(platform);
    MessagePlugin.info("已打开登录窗口，登录完成后点「我已登录」");
  } catch (e) {
    MessagePlugin.error(toAppError(e).message);
  }
}

async function confirm(platform: PlatformId) {
  try {
    await store.confirmConnection(platform);
    const next = new Set(pending.value);
    next.delete(platform);
    pending.value = next;
    MessagePlugin.success("已连接");
  } catch (e) {
    MessagePlugin.error(toAppError(e).message);
  }
}

async function disconnect(platform: PlatformId) {
  try {
    await store.disconnect(platform);
    MessagePlugin.success("已断开连接");
  } catch (e) {
    MessagePlugin.error(toAppError(e).message);
  }
}
</script>

<template>
  <div class="flex flex-col gap-3 p-4">
    <h3 class="text-base font-medium m-0">发布平台</h3>
    <div
      v-for="p in store.platforms"
      :key="p.platform"
      class="flex items-center justify-between border border-gray-200 rounded px-3 py-2"
    >
      <div class="flex items-center gap-2">
        <span class="font-medium">{{ PLATFORM_LABELS[p.platform] }}</span>
        <t-tag :theme="STATUS_THEME[p.status]" variant="light">
          {{ STATUS_TEXT[p.status] }}
        </t-tag>
        <span v-if="p.accountLabel" class="text-gray-500 text-sm">
          {{ p.accountLabel }}
        </span>
      </div>
      <div class="flex items-center gap-2">
        <template v-if="p.status !== 'Connected'">
          <t-button size="small" @click="connect(p.platform)">
            {{ p.status === "NeedReauth" ? "重新登录" : "登录" }}
          </t-button>
          <t-button
            v-if="pending.has(p.platform)"
            size="small"
            theme="primary"
            @click="confirm(p.platform)"
          >
            我已登录
          </t-button>
        </template>
        <t-button
          v-else
          size="small"
          theme="default"
          variant="outline"
          @click="disconnect(p.platform)"
        >
          断开
        </t-button>
      </div>
    </div>
  </div>
</template>
