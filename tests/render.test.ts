// T023 [US2]：render.ts 用 @md/core 渲染产出 HTML（与预览同源，SC-003）。
// 为保持确定性并避免加载 mermaid 等重依赖，mock 渲染核心。
import { describe, it, expect, vi } from "vitest";

const { modifyHtmlContent } = vi.hoisted(() => ({
  modifyHtmlContent: vi.fn(
    (md: string) => `<section style="font-size:16px">${md}</section>`,
  ),
}));

vi.mock("@md/core", () => ({
  initRenderer: vi.fn(() => ({ reset: vi.fn() })),
  applyTheme: vi.fn(async () => {}),
}));
vi.mock("@md/core/utils", () => ({ modifyHtmlContent }));
vi.mock("@md/shared/configs/style", () => ({
  defaultStyleConfig: {
    theme: "default",
    primaryColor: "#000",
    fontFamily: "sans",
    fontSize: "16px",
    headingStyles: {},
    isMacCodeBlock: true,
    isShowLineNumber: false,
    isCiteStatus: false,
    legend: "",
    isCountStatus: false,
  },
}));

import { renderArticleHtml } from "@/services/render";

describe("renderArticleHtml", () => {
  it("产出非空内联样式 HTML 并透传正文", async () => {
    const html = await renderArticleHtml("# 标题\n正文");
    expect(html).toContain("font-size");
    expect(html).toContain("# 标题");
    expect(modifyHtmlContent).toHaveBeenCalled();
  });

  it("空正文不抛错", async () => {
    const html = await renderArticleHtml("");
    expect(typeof html).toBe("string");
  });
});
