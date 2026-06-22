// T011 [US1]：publish store 连接状态机（FR-003/004/006）。
import { describe, it, expect, beforeEach, vi } from "vitest";
import { setActivePinia, createPinia } from "pinia";
import type { PlatformConnection } from "@/bindings/types";

const conn = (
  platform: PlatformConnection["platform"],
  status: PlatformConnection["status"],
  accountLabel: string | null = null,
): PlatformConnection => ({
  platform,
  status,
  accountLabel,
  lastCheckedAt: null,
});

const listPlatforms = vi.fn();
const connectPlatform = vi.fn();
const getPlatformStatus = vi.fn();
const disconnectPlatform = vi.fn();

vi.mock("@/bindings/commands", () => ({
  api: {
    listPlatforms: () => listPlatforms(),
    connectPlatform: (p: string) => connectPlatform(p),
    getPlatformStatus: (p: string) => getPlatformStatus(p),
    disconnectPlatform: (p: string) => disconnectPlatform(p),
  },
  onSyncProgress: vi.fn(async () => () => {}),
}));
vi.mock("@/services/render", () => ({ renderArticleHtml: vi.fn() }));

import { usePublishStore } from "@/stores/publish";

describe("publish store - connection", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.clearAllMocks();
  });

  it("refreshPlatforms 载入平台列表", async () => {
    listPlatforms.mockResolvedValue([
      conn("weixin", "Disconnected"),
      conn("zhihu", "Connected", "知乎用户"),
    ]);
    const store = usePublishStore();
    await store.refreshPlatforms();
    expect(store.platforms).toHaveLength(2);
    expect(store.isConnected("zhihu")).toBe(true);
    expect(store.isConnected("weixin")).toBe(false);
  });

  it("connect 后该平台更新为 Connected", async () => {
    listPlatforms.mockResolvedValue([conn("weixin", "Disconnected")]);
    connectPlatform.mockResolvedValue(conn("weixin", "Connected", "公众号"));
    const store = usePublishStore();
    await store.refreshPlatforms();
    await store.connect("weixin");
    expect(store.isConnected("weixin")).toBe(true);
    expect(store.platforms[0].accountLabel).toBe("公众号");
  });

  it("disconnect 后刷新为未连接", async () => {
    listPlatforms
      .mockResolvedValueOnce([conn("juejin", "Connected", "掘金")])
      .mockResolvedValueOnce([conn("juejin", "Disconnected")]);
    disconnectPlatform.mockResolvedValue(undefined);
    const store = usePublishStore();
    await store.refreshPlatforms();
    expect(store.isConnected("juejin")).toBe(true);
    await store.disconnect("juejin");
    expect(store.isConnected("juejin")).toBe(false);
  });
});
