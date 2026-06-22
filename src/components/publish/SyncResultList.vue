<script setup lang="ts">
// 002-multi-platform-publish US3：逐平台同步进度/结果/重试（FR-014/015/016）。
import { computed } from "vue";
import { usePublishStore } from "@/stores/publish";
import { PLATFORM_LABELS, type PlatformId, type SyncStatus } from "@/bindings/types";

const props = defineProps<{
  /** 重试所需的文章上下文（与发起同步一致）。 */
  articlePath: string;
  title: string;
  markdownBody: string;
  digest?: string;
  cover?: string;
}>();

const store = usePublishStore();

const items = computed(() => Object.values(store.jobs));

const STATUS_THEME: Record<SyncStatus, "primary" | "warning" | "success" | "danger"> = {
  Pending: "warning",
  Running: "primary",
  Success: "success",
  Failed: "danger",
};
const STATUS_TEXT: Record<SyncStatus, string> = {
  Pending: "等待中",
  Running: "同步中",
  Success: "成功",
  Failed: "失败",
};

async function retry(platform: PlatformId) {
  await store.retry(
    props.articlePath,
    props.title,
    props.markdownBody,
    platform,
    props.digest?.trim() || null,
    props.cover?.trim() || null,
  );
}
</script>

<template>
  <div v-if="items.length" class="flex flex-col gap-2">
    <div
      v-for="job in items"
      :key="job.id"
      class="flex items-center justify-between border border-gray-100 rounded px-3 py-2"
    >
      <div class="flex items-center gap-2 min-w-0">
        <span class="font-medium">{{ PLATFORM_LABELS[job.platform] }}</span>
        <t-tag :theme="STATUS_THEME[job.status]" variant="light">
          {{ STATUS_TEXT[job.status] }}
        </t-tag>
        <span
          v-if="job.status === 'Failed' && job.failureReason"
          class="text-red-500 text-sm truncate"
          :title="job.failureReason"
        >
          {{ job.failureReason }}
        </span>
      </div>
      <div class="flex items-center gap-2 shrink-0">
        <t-link
          v-if="job.status === 'Success' && job.draftRef?.url"
          theme="primary"
          :href="job.draftRef.url"
          target="_blank"
        >
          查看草稿
        </t-link>
        <t-button
          v-if="job.status === 'Failed'"
          size="small"
          variant="outline"
          @click="retry(job.platform)"
        >
          重试
        </t-button>
      </div>
    </div>
  </div>
</template>
