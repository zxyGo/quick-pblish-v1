<script setup lang="ts">
import { onMounted, ref } from "vue";
import { DialogPlugin, MessagePlugin } from "tdesign-vue-next";
import WorkspacePicker from "@/components/workspace/WorkspacePicker.vue";
import ArticleList from "@/components/article-list/ArticleList.vue";
import EditorPanel from "@/components/editor/EditorPanel.vue";
import { useWorkspaceStore } from "@/stores/workspace";
import { useArticlesStore } from "@/stores/articles";
import { useEditorStore } from "@/stores/editor";
import { api } from "@/bindings/commands";
import type { ArticleSummary } from "@/bindings/types";
import { toAppError } from "@/services/error";

const workspace = useWorkspaceStore();
const articles = useArticlesStore();
const editor = useEditorStore();

const newTitle = ref("");

onMounted(() => workspace.init());

async function openArticle(item: ArticleSummary) {
  if (editor.dirty && !(await confirmDiscard())) return;
  try {
    const content = await api.readArticle(item.relativePath);
    editor.load(content);
  } catch (e) {
    MessagePlugin.error(toAppError(e).message);
  }
}

function confirmDiscard(): Promise<boolean> {
  return new Promise((resolve) => {
    const dialog = DialogPlugin.confirm({
      header: "未保存的更改",
      body: "当前文章有未保存的更改，确定要放弃吗？",
      confirmBtn: "放弃更改",
      cancelBtn: "继续编辑",
      onConfirm: () => {
        dialog.destroy();
        resolve(true);
      },
      onCancel: () => resolve(false),
    });
  });
}

async function createNew() {
  if (!newTitle.value.trim()) {
    MessagePlugin.warning("请输入文章标题");
    return;
  }
  const safe = newTitle.value.trim().replace(/[\\/:*?"<>|]/g, "_");
  const relativePath = `${safe}.md`;
  try {
    const content = await api.createArticle(relativePath, newTitle.value.trim());
    editor.load(content);
    newTitle.value = "";
    await articles.refresh();
    MessagePlugin.success("已创建");
  } catch (e) {
    MessagePlugin.error(toAppError(e).message);
  }
}

async function save() {
  const result = await editor.save("abort");
  if (result.ok) {
    MessagePlugin.success("已保存");
    await articles.refresh();
    return;
  }
  if (result.conflict) {
    resolveConflict();
    return;
  }
  MessagePlugin.error(result.error ?? "保存失败");
}

function resolveConflict() {
  const dialog = DialogPlugin({
    header: "文件已被外部修改",
    body: "该文件在应用外部被修改。请选择处理方式：",
    confirmBtn: "覆盖外部更改",
    cancelBtn: "放弃本地并重载",
    onConfirm: async () => {
      dialog.destroy();
      const r = await editor.save("overwrite");
      if (r.ok) {
        MessagePlugin.success("已覆盖保存");
        await articles.refresh();
      }
    },
    onCancel: async () => {
      if (editor.open) {
        const fresh = await api.readArticle(editor.open.relativePath);
        editor.load(fresh);
        MessagePlugin.info("已重载外部版本");
      }
    },
  });
}
</script>

<template>
  <t-layout class="h-screen">
    <t-aside width="320px" class="flex flex-col overflow-hidden border-r border-gray-200">
      <div class="font-700 text-base p-3">Quick Publish</div>

      <template v-if="workspace.current">
        <div class="px-3">
          <span class="text-13px secondary-text" :title="workspace.current.path">
            {{ workspace.current.name }}
          </span>
          <WorkspacePicker />
        </div>
        <div class="flex gap-2 px-3 py-2">
          <t-input v-model="newTitle" placeholder="新文章标题" @enter="createNew" />
          <t-button theme="primary" @click="createNew">新建</t-button>
        </div>
        <ArticleList @select="openArticle" />
      </template>

      <template v-else>
        <t-empty description="请选择一个工作目录开始">
          <WorkspacePicker />
        </t-empty>
      </template>
    </t-aside>

    <t-layout>
      <t-header
        class="flex items-center justify-between px-4 border-b border-gray-200"
      >
        <span class="font-600">
          {{ editor.hasOpen ? editor.title : "未打开文章" }}
          <t-tag v-if="editor.dirty" theme="warning" size="small">未保存</t-tag>
        </span>
        <t-button v-if="editor.hasOpen" theme="primary" @click="save"
          >保存</t-button
        >
      </t-header>
      <t-content class="h-[calc(100vh-56px)] overflow-hidden">
        <EditorPanel
          v-if="editor.hasOpen"
          v-model="editor.body"
          @update:model-value="editor.markDirty"
        />
        <t-empty v-else description="新建或从左侧选择一篇文章" />
      </t-content>
    </t-layout>
  </t-layout>
</template>
