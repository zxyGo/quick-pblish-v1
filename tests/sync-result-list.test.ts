// T034 [US3]：SyncResultList 依 job 状态渲染进度/重试入口（FR-014/016）。
import { describe, it, expect, beforeEach, vi } from "vitest";
import { mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import type { SyncJob } from "@/bindings/types";

vi.mock("@/bindings/commands", () => ({
  api: {},
  onSyncProgress: vi.fn(async () => () => {}),
}));
vi.mock("@/services/render", () => ({ renderArticleHtml: vi.fn() }));

import { usePublishStore } from "@/stores/publish";
import SyncResultList from "@/components/publish/SyncResultList.vue";

// 测试中未注册 tdesign 全局组件，以简单元素 stub 保留插槽文本与按钮语义
const STUBS = {
  "t-tag": { template: "<span><slot /></span>" },
  "t-link": { template: "<a><slot /></a>" },
  "t-button": { template: "<button><slot /></button>" },
};

const job = (
  platform: SyncJob["platform"],
  status: SyncJob["status"],
  extra: Partial<SyncJob> = {},
): SyncJob => ({
  id: `${platform}-1`,
  articlePath: "a.md",
  platform,
  status,
  failureReason: null,
  draftRef: null,
  startedAt: null,
  finishedAt: null,
  ...extra,
});

describe("SyncResultList", () => {
  beforeEach(() => setActivePinia(createPinia()));

  it("展示逐平台状态，失败项显示重试按钮", () => {
    const store = usePublishStore();
    store.jobs = {
      weixin: job("weixin", "Success", {
        draftRef: { platform: "weixin", draftId: "d1", url: "https://x/d1" },
      }),
      zhihu: job("zhihu", "Failed", { failureReason: "auth 失效" }),
    } as Record<SyncJob["platform"], SyncJob>;

    const wrapper = mount(SyncResultList, {
      props: { articlePath: "a.md", title: "t", markdownBody: "x" },
      global: { stubs: STUBS },
    });

    const text = wrapper.text();
    expect(text).toContain("微信公众号");
    expect(text).toContain("知乎");
    expect(text).toContain("auth 失效");
    // 失败平台有「重试」按钮，成功平台没有
    const buttons = wrapper.findAll("button");
    expect(buttons.some((b) => b.text().includes("重试"))).toBe(true);
  });

  it("无任务时不渲染列表", () => {
    usePublishStore();
    const wrapper = mount(SyncResultList, {
      props: { articlePath: "a.md", title: "t", markdownBody: "x" },
      global: { stubs: STUBS },
    });
    expect(wrapper.find("div").exists()).toBe(false);
  });
});
