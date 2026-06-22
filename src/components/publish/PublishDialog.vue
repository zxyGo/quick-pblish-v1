<script setup lang="ts">
// 002-multi-platform-publish US2/US3：选平台一键同步为草稿（FR-007/008/012/013）。
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { MessagePlugin } from "tdesign-vue-next";
import { usePublishStore } from "@/stores/publish";
import { PLATFORM_LABELS, type PlatformId } from "@/bindings/types";
import { toAppError } from "@/services/error";
import SyncResultList from "./SyncResultList.vue";

const props = defineProps<{
  visible: boolean;
  articlePath: string;
  title: string;
  markdownBody: string;
}>();
const emit = defineEmits<{ "update:visible": [value: boolean] }>();

const store = usePublishStore();
const selected = ref<PlatformId[]>([]);

const connectedPlatforms = computed(() =>
  store.platforms.filter((p) => p.status === "Connected"),
);

onMounted(async () => {
  await store.initProgress();
  await store.refreshPlatforms();
});
onUnmounted(() => store.disposeProgress());

// 打开对话框时刷新平台状态，默认勾选已连接平台
watch(
  () => props.visible,
  async (v) => {
    if (v) {
      await store.refreshPlatforms();
      selected.value = connectedPlatforms.value.map((p) => p.platform);
    }
  },
);

async function doSync() {
  // 前端预拦截（FR-012 第一道闸）：未选已连接平台则提示
  const targets = selected.value.filter((p) => store.isConnected(p));
  if (targets.length === 0) {
    MessagePlugin.warning("请先连接并选择至少一个平台");
    return;
  }
  try {
    const jobs = await store.syncArticle(
      props.articlePath,
      props.title,
      props.markdownBody,
      targets,
    );
    const ok = jobs.filter((j) => j.status === "Success").length;
    if (ok === jobs.length) MessagePlugin.success(`同步成功（${ok}/${jobs.length}）`);
    else MessagePlugin.warning(`部分平台失败（成功 ${ok}/${jobs.length}），可重试`);
  } catch (e) {
    MessagePlugin.error(toAppError(e).message);
  }
}
</script>

<template>
  <t-dialog
    :visible="props.visible"
    header="同步到平台草稿"
    :footer="false"
    width="560px"
    @close="emit('update:visible', false)"
  >
    <div class="flex flex-col gap-4">
      <div class="text-sm text-gray-500 truncate">文章：{{ props.title }}</div>

      <div v-if="connectedPlatforms.length" class="flex flex-col gap-2">
        <span class="text-sm font-medium">选择目标平台</span>
        <t-checkbox-group v-model="selected">
          <t-checkbox
            v-for="p in connectedPlatforms"
            :key="p.platform"
            :value="p.platform"
          >
            {{ PLATFORM_LABELS[p.platform] }}
            <span v-if="p.accountLabel" class="text-gray-400">
              （{{ p.accountLabel }}）
            </span>
          </t-checkbox>
        </t-checkbox-group>
      </div>
      <div v-else class="text-sm text-orange-500">
        暂无已连接平台，请先在"发布平台"面板连接。
      </div>

      <div class="flex justify-end">
        <t-button :loading="store.syncing" @click="doSync">一键同步</t-button>
      </div>

      <SyncResultList
        :article-path="props.articlePath"
        :title="props.title"
        :markdown-body="props.markdownBody"
      />
    </div>
  </t-dialog>
</template>
