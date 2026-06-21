<script setup lang="ts">
import { ref } from "vue";
import type { DropdownOption } from "tdesign-vue-next";

/**
 * 编辑器快捷菜单栏（参考 doocs/md 的顶部菜单：文件 / 编辑 / 格式 / 插入 / 样式 / 帮助）。
 *
 * 菜单项值编码为 `group|action|arg?`：
 * - `editor|*` → 透传给 EditorPanel.exec(action, arg)（格式、插入、撤销/重做、样式）
 * - `file|*`   → 文件级操作（新建/保存），由 MainView 处理
 * - `help|*`   → 帮助对话框，本组件内部处理
 */
const emit = defineEmits<{
  "editor-action": [action: string, arg?: string];
  "file-action": [action: string];
}>();

// const fileOptions: DropdownOption[] = [
//   { content: "新建文章", value: "file|new" },
//   { content: "保存", value: "file|save" },
// ];

const editOptions: DropdownOption[] = [
  { content: "撤销", value: "editor|undo" },
  { content: "重做", value: "editor|redo" },
  { content: "复制全文", value: "editor|copyAll" },
  { content: "清空", value: "editor|clear" },
];

const formatOptions: DropdownOption[] = [
  { content: "加粗", value: "editor|bold" },
  { content: "斜体", value: "editor|italic" },
  { content: "删除线", value: "editor|strike" },
  { content: "行内代码", value: "editor|code" },
  { content: "超链接", value: "editor|link" },
  {
    content: "标题",
    value: "editor|heading-group",
    children: [
      { content: "一级标题", value: "editor|heading|1" },
      { content: "二级标题", value: "editor|heading|2" },
      { content: "三级标题", value: "editor|heading|3" },
    ],
  },
  { content: "引用", value: "editor|quote" },
  { content: "无序列表", value: "editor|ul" },
  { content: "有序列表", value: "editor|ol" },
];

const insertOptions: DropdownOption[] = [
  { content: "图片", value: "editor|image" },
  { content: "表格", value: "editor|table" },
  { content: "代码块", value: "editor|codeblock" },
  { content: "分割线", value: "editor|hr" },
];

const styleOptions: DropdownOption[] = [
  {
    content: "主题",
    value: "editor|theme-group",
    children: [
      { content: "经典", value: "editor|theme|default" },
      { content: "优雅", value: "editor|theme|grace" },
      { content: "简洁", value: "editor|theme|simple" },
    ],
  },
  {
    content: "主题色",
    value: "editor|color-group",
    children: [
      { content: "经典蓝", value: "editor|color|#0F4C81" },
      { content: "翡翠绿", value: "editor|color|#009874" },
      { content: "活力橘", value: "editor|color|#FA5151" },
      { content: "薰衣紫", value: "editor|color|#92617E" },
      { content: "石墨黑", value: "editor|color|#333333" },
    ],
  },
  {
    content: "字号",
    value: "editor|fontSize-group",
    children: [
      { content: "14px", value: "editor|fontSize|14px" },
      { content: "15px", value: "editor|fontSize|15px" },
      { content: "16px", value: "editor|fontSize|16px" },
      { content: "17px", value: "editor|fontSize|17px" },
      { content: "18px", value: "editor|fontSize|18px" },
    ],
  },
  {
    content: "字体",
    value: "editor|fontFamily-group",
    children: [
      {
        content: "无衬线",
        value:
          "editor|fontFamily|-apple-system-font,BlinkMacSystemFont, Helvetica Neue, PingFang SC, Hiragino Sans GB , Microsoft YaHei UI , Microsoft YaHei ,Arial,sans-serif",
      },
      {
        content: "衬线",
        value:
          "editor|fontFamily|Optima-Regular, Optima, PingFangSC-light, PingFangTC-light, 'PingFang SC', Cambria, Cochin, Georgia, Times, 'Times New Roman', serif",
      },
      { content: "等宽", value: "editor|fontFamily|Menlo, Monaco, 'Courier New', monospace" },
    ],
  },
];

const helpOptions: DropdownOption[] = [
  { content: "Markdown 语法", value: "help|syntax" },
  { content: "关于", value: "help|about" },
];

const showSyntax = ref(false);
const showAbout = ref(false);

function onClick(option: DropdownOption) {
  // 父级（带 children 的）分组项不携带可执行动作，忽略。
  const value = String(option.value);
  const [group, action, arg] = value.split("|");
  if (action.endsWith("-group")) return;
  if (group === "editor") {
    emit("editor-action", action, arg);
  } else if (group === "file") {
    emit("file-action", action);
  } else if (group === "help") {
    if (action === "syntax") showSyntax.value = true;
    else if (action === "about") showAbout.value = true;
  }
}

const menus: { label: string; options: DropdownOption[] }[] = [
  // { label: "文件", options: fileOptions },
  { label: "编辑", options: editOptions },
  { label: "格式", options: formatOptions },
  { label: "插入", options: insertOptions },
  { label: "样式", options: styleOptions },
  { label: "帮助", options: helpOptions },
];
</script>

<template>
  <div class="flex items-center gap-1 px-2 py-1 border-b border-gray-200 bg-gray-50 shrink-0">
    <t-dropdown
      v-for="menu in menus"
      :key="menu.label"
      :options="menu.options"
      trigger="click"
      placement="bottom-left"
      @click="onClick"
    >
      <t-button variant="text" size="small" class="px-2">{{ menu.label }}</t-button>
    </t-dropdown>

    <t-dialog v-model:visible="showAbout" header="关于" :footer="false" width="420px">
      <div class="text-sm leading-relaxed">
        <p class="font-600">Quick Publish</p>
        <p class="mt-2">本地优先的 Markdown 创作与多平台发布工具。</p>
        <p class="mt-2 muted">
          编辑器样式预览由
          <a href="https://github.com/doocs/md" target="_blank" rel="noreferrer">doocs/md</a>
          渲染核心（WTFPL）驱动，特此致谢。
        </p>
      </div>
    </t-dialog>

    <t-dialog v-model:visible="showSyntax" header="Markdown 语法" :footer="false" width="480px">
      <div class="text-sm leading-relaxed font-mono whitespace-pre-wrap">
# 标题（# ~ ######）
**加粗**  *斜体*  ~~删除线~~  `行内代码`
[链接](https://example.com)
![图片](路径)
> 引用
- 无序列表
1. 有序列表
| 表头 | 表头 |
| --- | --- |
| 单元 | 单元 |
```代码块```
---（分割线）
      </div>
    </t-dialog>
  </div>
</template>
