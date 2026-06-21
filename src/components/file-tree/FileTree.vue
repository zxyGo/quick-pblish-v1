<script setup lang="ts">
import { onMounted, ref } from "vue";
import { DialogPlugin, MessagePlugin } from "tdesign-vue-next";
import { api } from "@/bindings/commands";
import type { FileNode } from "@/bindings/types";
import { toAppError } from "@/services/error";

const emit = defineEmits<{ open: [relativePath: string] }>();

const nodes = ref<FileNode[]>([]);
const loading = ref(false);

async function refresh() {
  loading.value = true;
  try {
    const root = await api.getFileTree();
    nodes.value = root.children;
  } catch (e) {
    MessagePlugin.error(toAppError(e).message);
  } finally {
    loading.value = false;
  }
}

onMounted(refresh);
defineExpose({ refresh });

function onClick(context: { node: { data: FileNode } }) {
  const data = context.node.data;
  if (data.kind === "file" && data.isArticle) {
    emit("open", data.relativePath);
  }
}

function confirmDelete(node: FileNode) {
  const dialog = DialogPlugin.confirm({
    header: "删除确认",
    body: `确定将「${node.name}」移入系统回收站吗？可从回收站恢复。`,
    confirmBtn: "移入回收站",
    cancelBtn: "取消",
    onConfirm: async () => {
      dialog.destroy();
      try {
        await api.deletePath(node.relativePath);
        MessagePlugin.success("已移入回收站");
        await refresh();
      } catch (e) {
        MessagePlugin.error(toAppError(e).message);
      }
    },
  });
}
</script>

<template>
  <t-loading :loading="loading" size="small" class="h-full">
    <t-tree
      v-if="nodes.length"
      :data="nodes"
      :keys="{ value: 'relativePath', label: 'name', children: 'children' }"
      hover
      expand-on-click-node
      class="p-2"
      @click="onClick"
    >
      <template #operations="{ node }">
        <t-button
          size="small"
          theme="danger"
          variant="text"
          @click.stop="confirmDelete(node.data)"
        >
          删除
        </t-button>
      </template>
    </t-tree>
    <t-empty v-else description="目录为空" />
  </t-loading>
</template>
