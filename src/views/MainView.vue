<script setup lang="ts">
import { onMounted, onUnmounted, ref } from "vue";
import { DialogPlugin, MessagePlugin } from "tdesign-vue-next";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import WorkspacePicker from "@/components/workspace/WorkspacePicker.vue";
import ArticleList from "@/components/article-list/ArticleList.vue";
import FileTree from "@/components/file-tree/FileTree.vue";
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
const sideTab = ref("articles");
const fileTree = ref<InstanceType<typeof FileTree> | null>(null);

let unlisten: UnlistenFn | null = null;

onMounted(async () => {
  await workspace.init();
  // 监听外部文件变化，刷新列表与文件树（T036 / FR-019 一致性）
  unlisten = await listen("workspace_changed", () => {
    articles.refresh();
    fileTree.value?.refresh();
  });
});

onUnmounted(() => unlisten?.());

async function openArticle(relativePath: string) {
  if (editor.dirty && !(await confirmDiscard())) return;
  try {
    const content = await api.readArticle(relativePath);
    editor.load(content);
  } catch (e) {
    MessagePlugin.error(toAppError(e).message);
  }
}

function openFromList(item: ArticleSummary) {
  openArticle(item.relativePath);
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
    fileTree.value?.refresh();
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
        <t-tabs v-model="sideTab" class="flex-1 min-h-0 flex flex-col">
          <t-tab-panel value="articles" label="文章">
            <ArticleList @select="openFromList" />
          </t-tab-panel>
          <t-tab-panel value="files" label="文件">
            <FileTree ref="fileTree" @open="openArticle" />
          </t-tab-panel>
        </t-tabs>
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

      <div
        v-if="editor.hasOpen"
        class="flex items-center gap-2 px-4 py-2 border-b border-gray-200"
      >
        <span class="text-xs muted">标签</span>
        <t-tag-input
          v-model="editor.tags"
          placeholder="回车添加标签"
          class="max-w-120"
          @change="editor.markDirty"
        />
      </div>

      <t-content class="flex-1 min-h-0 overflow-hidden">
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
