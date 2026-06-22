// 002-multi-platform-publish：文章渲染服务（render.ts，FR-009 / SC-003）。
//
// 复用与编辑器预览**完全同源**的 doocs/md 渲染管线（initRenderer + modifyHtmlContent），
// 把 Markdown 正文渲染为带内联样式的 HTML，作为各平台 adapter 的输入。同源保证草稿样式
// 与应用内预览一致（SC-003 抽样一致率 ≥95%）。参见 EditorPanel.vue 的渲染实现。

import { applyTheme, initRenderer } from "@md/core";
import { modifyHtmlContent } from "@md/core/utils";
import { defaultStyleConfig } from "@md/shared/configs/style";

const renderer = initRenderer({
  isMacCodeBlock: defaultStyleConfig.isMacCodeBlock,
  isShowLineNumber: defaultStyleConfig.isShowLineNumber,
});

let themeReady = false;

async function ensureTheme(): Promise<void> {
  if (themeReady) return;
  await applyTheme({
    themeName: defaultStyleConfig.theme as string,
    variables: {
      primaryColor: defaultStyleConfig.primaryColor as string,
      fontFamily: defaultStyleConfig.fontFamily as string,
      fontSize: defaultStyleConfig.fontSize as string,
      headingStyles: defaultStyleConfig.headingStyles,
    },
  });
  themeReady = true;
}

/**
 * 把文章正文（Markdown）渲染为带内联样式的 HTML，供同步到各平台。
 * 与 EditorPanel 预览同源，确保样式保真。
 */
export async function renderArticleHtml(markdownBody: string): Promise<string> {
  await ensureTheme();
  renderer.reset({
    citeStatus: defaultStyleConfig.isCiteStatus,
    legend: defaultStyleConfig.legend,
    countStatus: defaultStyleConfig.isCountStatus,
    isMacCodeBlock: defaultStyleConfig.isMacCodeBlock,
    isShowLineNumber: defaultStyleConfig.isShowLineNumber,
    themeMode: "light",
  });
  return modifyHtmlContent(markdownBody || "", renderer);
}
