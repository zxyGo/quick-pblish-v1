<script setup lang="ts">
import { computed, onMounted, reactive, ref } from "vue";
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
 * 注入主题样式（作用域 #output），`modifyHtmlContent` 将正文渲染为带样式 HTML。
 * 文件读写一律由 Tauri 后端负责，doocs/md 仅负责"编辑与预览"，不触碰文件系统。
 * props/emit 契约保持不变：输入 `modelValue`(正文)，输出 `update:modelValue`。
 *
 * 编辑命令（exec）通过 `document.execCommand('insertText')` 作用于 textarea，
 * 既能整合浏览器原生撤销/重做栈，又会触发 input 事件让 v-model 同步。
 * 详见 specs/001-local-content-management/research.md 第 1 节。
 */
const props = defineProps<{ modelValue: string }>();
const emit = defineEmits<{ "update:modelValue": [value: string] }>();

// 渲染器实例（doocs/md 渲染核心）。
const renderer = initRenderer({
  isMacCodeBlock: defaultStyleConfig.isMacCodeBlock,
  isShowLineNumber: defaultStyleConfig.isShowLineNumber,
});

// 当前预览样式（样式菜单可改），随之重新注入主题。
const style = reactive({
  themeName: defaultStyleConfig.theme as string,
  primaryColor: defaultStyleConfig.primaryColor as string,
  fontFamily: defaultStyleConfig.fontFamily as string,
  fontSize: defaultStyleConfig.fontSize as string,
});

// 主题注入是否就绪——注入后触发首屏预览重算。
const themeReady = ref(false);

async function applyCurrentTheme() {
  // 向 document.head 注入作用域为 #output 的主题样式（CSS 变量 + 主题 CSS）。
  await applyTheme({
    themeName: style.themeName,
    variables: {
      primaryColor: style.primaryColor,
      fontFamily: style.fontFamily,
      fontSize: style.fontSize,
      headingStyles: defaultStyleConfig.headingStyles,
    },
  });
  themeReady.value = true;
}

onMounted(applyCurrentTheme);

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

/** 用 execCommand 替换 textarea 当前选区，保留原生撤销栈并触发 input 同步。 */
function replaceSelection(text: string): void {
  const el = textarea.value;
  if (!el) {
    emit("update:modelValue", (props.modelValue ?? "") + text);
    return;
  }
  el.focus();
  document.execCommand("insertText", false, text);
}

/** 在选区两侧包裹标记（加粗/斜体/删除线/行内代码）。空选区则把光标置于标记中间。 */
function wrapSelection(prefix: string, suffix: string = prefix): void {
  const el = textarea.value;
  if (!el) return;
  el.focus();
  const start = el.selectionStart;
  const end = el.selectionEnd;
  const selected = el.value.slice(start, end);
  replaceSelection(`${prefix}${selected}${suffix}`);
  if (start === end) {
    const pos = start + prefix.length;
    el.setSelectionRange(pos, pos);
  } else {
    const s = start + prefix.length;
    el.setSelectionRange(s, s + selected.length);
  }
}

/** 给选区覆盖的每一行加前缀（标题/引用/无序列表/有序列表）。 */
function prefixLines(kind: "ul" | "ol" | "quote" | "heading", level = 1): void {
  const el = textarea.value;
  if (!el) return;
  el.focus();
  const value = el.value;
  const lineStart = value.lastIndexOf("\n", el.selectionStart - 1) + 1;
  let lineEnd = value.indexOf("\n", el.selectionEnd);
  if (lineEnd === -1) lineEnd = value.length;
  const block = value.slice(lineStart, lineEnd);
  const newBlock = block
    .split("\n")
    .map((line, i) => {
      switch (kind) {
        case "ul":
          return `- ${line}`;
        case "ol":
          return `${i + 1}. ${line}`;
        case "quote":
          return `> ${line}`;
        case "heading":
          return `${"#".repeat(level)} ${line.replace(/^#{1,6}\s+/, "")}`;
      }
    })
    .join("\n");
  el.setSelectionRange(lineStart, lineEnd);
  replaceSelection(newBlock);
}

function insertLink(): void {
  const el = textarea.value;
  if (!el) return;
  el.focus();
  const selected = el.value.slice(el.selectionStart, el.selectionEnd) || "链接文字";
  replaceSelection(`[${selected}](https://)`);
}

const TABLE_SNIPPET = `\n| 列 1 | 列 2 |\n| --- | --- |\n| 内容 | 内容 |\n`;
const CODEBLOCK_SNIPPET = "\n```js\n\n```\n";

/** 选择本地图片 → 复制进工作目录 assets/ → 光标处插入相对路径引用（FR-014a）。 */
async function insertImage() {
  const selected = await open({
    multiple: false,
    filters: [{ name: "图片", extensions: ["png", "jpg", "jpeg", "gif", "webp", "svg"] }],
  });
  if (typeof selected !== "string") return;
  try {
    const asset = await api.importAsset(selected);
    replaceSelection(`![${asset.fileName}](${asset.relativePath})`);
    MessagePlugin.success("已插入图片");
  } catch (e) {
    MessagePlugin.error(toAppError(e).message);
  }
}

async function copyAll() {
  try {
    await navigator.clipboard.writeText(props.modelValue ?? "");
    MessagePlugin.success("已复制全文");
  } catch {
    MessagePlugin.error("复制失败");
  }
}

function clearAll() {
  const el = textarea.value;
  if (!el) return;
  el.focus();
  el.select();
  document.execCommand("insertText", false, "");
}

/**
 * 编辑器命令统一入口，由菜单栏调用。
 * 格式/插入命令作用于选区；样式命令重新注入主题；撤销/重做走原生栈。
 */
async function exec(action: string, arg?: string): Promise<void> {
  switch (action) {
    case "bold":
      return wrapSelection("**");
    case "italic":
      return wrapSelection("*");
    case "strike":
      return wrapSelection("~~");
    case "code":
      return wrapSelection("`");
    case "link":
      return insertLink();
    case "heading":
      return prefixLines("heading", Number(arg) || 1);
    case "quote":
      return prefixLines("quote");
    case "ul":
      return prefixLines("ul");
    case "ol":
      return prefixLines("ol");
    case "table":
      return replaceSelection(TABLE_SNIPPET);
    case "codeblock":
      return replaceSelection(CODEBLOCK_SNIPPET);
    case "hr":
      return replaceSelection("\n---\n");
    case "image":
      return insertImage();
    case "undo":
      textarea.value?.focus();
      document.execCommand("undo");
      return;
    case "redo":
      textarea.value?.focus();
      document.execCommand("redo");
      return;
    case "copyAll":
      return copyAll();
    case "clear":
      return clearAll();
    case "theme":
      style.themeName = arg ?? style.themeName;
      return applyCurrentTheme();
    case "color":
      style.primaryColor = arg ?? style.primaryColor;
      return applyCurrentTheme();
    case "fontFamily":
      style.fontFamily = arg ?? style.fontFamily;
      return applyCurrentTheme();
    case "fontSize":
      style.fontSize = arg ?? style.fontSize;
      return applyCurrentTheme();
  }
}

defineExpose({ insertImage, exec });
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
