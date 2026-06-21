<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { open } from "@tauri-apps/plugin-dialog";
import { MessagePlugin } from "tdesign-vue-next";
import { applyTheme, initRenderer } from "@md/core";
import { modifyHtmlContent } from "@md/core/utils";
import { defaultStyleConfig } from "@md/shared/configs/style";
import { api } from "@/bindings/commands";
import { toAppError } from "@/services/error";

/**
 * Markdown 编辑器 + 样式预览组件。
 *
 * doocs/md 集成（T022 已接入）：右侧样式预览复用 doocs/md 的渲染管线——
 * 渲染核心（@md/core / @md/shared）以源码形式 vendored 于 `vendor/doocs-md/`，
 * 通过 pnpm workspace 链接。本组件用 `initRenderer` 构建渲染器、`applyTheme`
 * 注入默认主题样式（作用域 #output），`modifyHtmlContent` 将正文渲染为带样式 HTML。
 * 文件读写一律由 Tauri 后端负责，doocs/md 仅负责"编辑与预览"，不触碰文件系统。
 * props/emit 契约保持不变：输入 `modelValue`(正文)，输出 `update:modelValue`。
 * 详见 specs/001-local-content-management/research.md 第 1 节。
 */
const props = defineProps<{ modelValue: string }>();
const emit = defineEmits<{ "update:modelValue": [value: string] }>();

// 渲染器实例（doocs/md 渲染核心）。
const renderer = initRenderer({
  isMacCodeBlock: defaultStyleConfig.isMacCodeBlock,
  isShowLineNumber: defaultStyleConfig.isShowLineNumber,
});

// 主题注入是否就绪——注入后触发首屏预览重算。
const themeReady = ref(false);

onMounted(async () => {
  // 向 document.head 注入作用域为 #output 的默认主题样式（CSS 变量 + 主题 CSS）。
  await applyTheme({
    themeName: defaultStyleConfig.theme,
    variables: {
      primaryColor: defaultStyleConfig.primaryColor,
      fontFamily: defaultStyleConfig.fontFamily,
      fontSize: defaultStyleConfig.fontSize,
      headingStyles: defaultStyleConfig.headingStyles,
    },
  });
  themeReady.value = true;
});

const rendered = computed(() => {
  // 依赖 themeReady：主题注入完成后让预览重算一次。
  void themeReady.value;
  // 每次渲染前重置渲染器内部状态（脚注计数等），避免跨次累积。
  renderer.reset({
    citeStatus: defaultStyleConfig.isCiteStatus,
    legend: defaultStyleConfig.legend,
    countStatus: defaultStyleConfig.isCountStatus,
    isMacCodeBlock: defaultStyleConfig.isMacCodeBlock,
    isShowLineNumber: defaultStyleConfig.isShowLineNumber,
    themeMode: "light",
  });
  return modifyHtmlContent(props.modelValue || "", renderer);
});

const textarea = ref<HTMLTextAreaElement | null>(null);

function onInput(e: Event) {
  emit("update:modelValue", (e.target as HTMLTextAreaElement).value);
}

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

/** 选择本地图片 → 复制进工作目录 assets/ → 光标处插入相对路径引用（FR-014a）。 */
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

defineExpose({ insertImage });
</script>

<template>
  <div class="grid grid-cols-2 gap-px h-full bg-gray-200">
    <textarea
      ref="textarea"
      class="h-full overflow-auto p-4 box-border bg-white border-none outline-none resize-none font-mono text-sm leading-relaxed"
      :value="modelValue"
      placeholder="在此输入 Markdown..."
      @input="onInput"
    />
    <!-- doocs/md 样式预览区：id="output" 与注入主题 CSS 的作用域匹配 -->
    <!-- eslint-disable-next-line vue/no-v-html -->
    <div id="output" class="h-full overflow-auto p-4 box-border bg-white text-left" v-html="rendered" />
  </div>
</template>
