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
 * 取注入到文档的主题 CSS（`applyTheme` 注入的 `#md-theme`，作用域 `#output`），
 * 并剥掉 `#output` 作用域前缀，使规则能匹配「脱离 #output 祖先」的离屏片段元素。
 * 对齐 doocs/md `share-styles.ts::stripOutputScope` 的复制/导出处理。
 * 注：vendored `applyTheme` 注入前已 `processCSS` 解析掉 `var()`，故无需再处理变量。
 */
function getThemeCssForInline(): string {
  const el = document.querySelector("#md-theme") as HTMLStyleElement | null;
  if (!el?.textContent) return "";
  return el.textContent
    .replace(/#output\s*\{/g, "body {")
    .replace(/#output\s+/g, "")
    .replace(/^#output\s*/gm, "");
}

/**
 * 把 class 驱动的渲染结果「内联化」：用主题样式表的**选择器匹配**把每条声明写回元素
 * inline `style`（等价于 doocs/md 用 juice 做的事，但用浏览器原生 CSSStyleSheet 实现，
 * 不引重依赖）。
 *
 * 必要性：doocs/md 新主题系统下 renderer 产出的 HTML 几乎只有 class 名、视觉样式全在注入
 * 的样式表里（不随 HTML 走）。微信编辑器会清除外部样式表与 class，直接送原始 HTML 会丢失
 * 全部样式（预览有样式、草稿是裸文本）。按选择器内联后样式随元素走，脱离样式表也保真。
 *
 * 相比 `getComputedStyle` 全量 dump：只内联 CSS **真正声明过**的属性，产物干净、不含数百条
 * 默认值，避免被微信编辑器整体拒收。
 *
 * 非浏览器环境（纯单测无 DOM）原样返回。
 */
function inlineThemeStyles(html: string): string {
  if (typeof document === "undefined" || typeof CSSStyleSheet === "undefined") {
    return html;
  }
  const host = document.createElement("div");
  host.setAttribute("style", "position:fixed;left:-99999px;top:0;");
  host.innerHTML = html;
  document.body.appendChild(host);
  try {
    const css = getThemeCssForInline();
    if (css) {
      // 记录元素原有 inline 样式（如表格 text-align），最后回写以保证其优先级（对齐 juice：
      // 源码内联样式优先于样式表内联）。
      const originalInline = new Map<HTMLElement, string>();
      for (const el of host.querySelectorAll<HTMLElement>("[style]")) {
        originalInline.set(el, el.getAttribute("style") ?? "");
      }

      let sheet: CSSStyleSheet | null = null;
      try {
        sheet = new CSSStyleSheet();
        sheet.replaceSync(css);
      } catch {
        sheet = null; // 环境不支持 Constructable StyleSheet → 跳过内联
      }

      if (sheet) {
        // 按样式表源码顺序应用：同特异性下后者覆盖前者（主题 CSS 已把标题覆盖样式排在主题之后）。
        for (const rule of Array.from(sheet.cssRules)) {
          if (!(rule instanceof CSSStyleRule)) continue;
          let targets: NodeListOf<HTMLElement>;
          try {
            targets = host.querySelectorAll<HTMLElement>(rule.selectorText);
          } catch {
            continue; // 选择器含片段中无意义的伪类等 → 跳过
          }
          if (targets.length === 0) continue;
          const decl = rule.style;
          targets.forEach((t) => {
            for (let i = 0; i < decl.length; i++) {
              const prop = decl[i];
              t.style.setProperty(
                prop,
                decl.getPropertyValue(prop),
                decl.getPropertyPriority(prop),
              );
            }
          });
        }
      }

      // 回写原有 inline 样式，使其压过样式表内联（juice 行为）。
      for (const [el, style] of originalInline) {
        for (const part of style.split(";")) {
          const idx = part.indexOf(":");
          if (idx < 0) continue;
          const prop = part.slice(0, idx).trim();
          const val = part.slice(idx + 1).trim();
          if (prop && val) el.style.setProperty(prop, val);
        }
      }

      // 图片 width/height 属性转 inline 样式（微信编辑器更认 style，对齐 solveWeChatImage）。
      for (const img of host.querySelectorAll("img")) {
        const w = img.getAttribute("width");
        const h = img.getAttribute("height");
        if (w) {
          img.removeAttribute("width");
          img.style.width = /^\d+$/.test(w) ? `${w}px` : w;
        }
        if (h) {
          img.removeAttribute("height");
          img.style.height = /^\d+$/.test(h) ? `${h}px` : h;
        }
      }
    }

    // 变量兜底（shell 变量来自宿主 :root，片段内未定义 → 替换为固定值，对齐 doocs/md）。
    return host.innerHTML
      .replace(/hsl\(var\(--foreground\)\)/g, "#3f3f3f")
      .replace(/var\(--blockquote-background\)/g, "#f7f7f7");
  } finally {
    document.body.removeChild(host);
  }
}

/**
 * 把文章正文（Markdown）渲染为带内联样式的 HTML，供同步到各平台。
 * 与 EditorPanel 预览同源（同一渲染管线 + 同一作用域主题），再按选择器内联样式确保跨平台保真。
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
  const html = modifyHtmlContent(markdownBody || "", renderer);
  return inlineThemeStyles(html);
}
