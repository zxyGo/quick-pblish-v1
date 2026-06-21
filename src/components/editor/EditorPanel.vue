<script setup lang="ts">
import { computed, ref } from "vue";
import MarkdownIt from "markdown-it";
import { open } from "@tauri-apps/plugin-dialog";
import { MessagePlugin } from "tdesign-vue-next";
import { api } from "@/bindings/commands";
import { toAppError } from "@/services/error";

/**
 * Markdown 编辑器 + 样式预览组件。
 *
 * doocs/md 集成接缝：当前实现使用轻量 markdown-it 渲染预览，作为可用的占位。
 * 后续 T022 将把 doocs/md 的编辑器与样式渲染管线接入此处（保持相同的 props/emit 契约：
 * 输入 `modelValue`(正文)，输出 `update:modelValue`），文件读写一律由后端负责。
 * 详见 specs/001-local-content-management/research.md 第 1 节。
 */
const props = defineProps<{ modelValue: string }>();
const emit = defineEmits<{ "update:modelValue": [value: string] }>();

const md = new MarkdownIt({ html: false, linkify: true, breaks: true });
const rendered = computed(() => md.render(props.modelValue || ""));

const textarea = ref<HTMLTextAreaElement | null>(null);

function onInput(e: Event) {
  emit("update:modelValue", (e.target as HTMLTextAreaElement).value);
}

/** 在光标处插入文本并回传更新（FR-014a 插图用）。 */
function insertAtCursor(snippet: string) {
  const el = textarea.value;
  const value = props.modelValue ?? "";
  if (!el) {
    emit("update:modelValue", value + snippet);
    return;
  }
  const start = el.selectionStart ?? value.length;
  const end = el.selectionEnd ?? value.length;
  emit("update:modelValue", value.slice(0, start) + snippet + value.slice(end));
}

async function insertImage() {
  const selected = await open({
    multiple: false,
    filters: [{ name: "图片", extensions: ["png", "jpg", "jpeg", "gif", "webp", "svg"] }],
  });
  if (typeof selected !== "string") return;
  try {
    const asset = await api.importAsset(selected);
    insertAtCursor(`![${asset.fileName}](${asset.relativePath})`);
    MessagePlugin.success("已插入图片");
  } catch (e) {
    MessagePlugin.error(toAppError(e).message);
  }
}
</script>

<template>
  <div class="flex flex-col h-full">
    <div class="flex items-center gap-2 px-3 py-1.5 border-b border-gray-200">
      <t-button size="small" variant="outline" @click="insertImage">
        插入图片
      </t-button>
    </div>
    <div class="grid grid-cols-2 gap-px flex-1 min-h-0 bg-gray-200">
      <textarea
        ref="textarea"
        class="h-full overflow-auto p-4 box-border bg-white border-none outline-none resize-none font-mono text-sm leading-relaxed"
        :value="modelValue"
        placeholder="在此输入 Markdown..."
        @input="onInput"
      />
      <!-- eslint-disable-next-line vue/no-v-html -->
      <div
        class="markdown-preview h-full overflow-auto p-4 box-border bg-white text-left"
        v-html="rendered"
      />
    </div>
  </div>
</template>

<style scoped>
/* 原子类无法穿透 v-html 渲染出的子元素，此处保留最小 :deep() 样式 */
.markdown-preview :deep(pre) {
  background: #f6f8fa;
  padding: 12px;
  border-radius: 6px;
  overflow: auto;
}
.markdown-preview :deep(code) {
  background: #f6f8fa;
  padding: 2px 4px;
  border-radius: 4px;
}
</style>
