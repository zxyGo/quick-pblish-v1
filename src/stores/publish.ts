// 002-multi-platform-publish：发布状态（Pinia）。
// 管理平台连接状态与同步任务，订阅后端进度事件归并 job 状态（FR-003/014/015/016）。

import { defineStore } from "pinia";
import { ref } from "vue";
import type { UnlistenFn } from "@tauri-apps/api/event";
import { api, onSyncProgress } from "@/bindings/commands";
import { renderArticleHtml } from "@/services/render";
import type {
  PlatformConnection,
  PlatformId,
  SyncJob,
  SyncRecord,
} from "@/bindings/types";

export const usePublishStore = defineStore("publish", () => {
  const platforms = ref<PlatformConnection[]>([]);
  /** 当前批次的同步任务，按平台归并（每平台最新一条）。 */
  const jobs = ref<Record<PlatformId, SyncJob>>(
    {} as Record<PlatformId, SyncJob>,
  );
  const syncing = ref(false);
  const history = ref<SyncRecord[]>([]);

  let unlisten: UnlistenFn | null = null;

  /** 初始化进度事件订阅（应在应用启动或面板挂载时调用一次）。 */
  async function initProgress(): Promise<void> {
    if (unlisten) return;
    unlisten = await onSyncProgress((job) => {
      jobs.value = { ...jobs.value, [job.platform]: job };
    });
  }

  function disposeProgress(): void {
    unlisten?.();
    unlisten = null;
  }

  async function refreshPlatforms(): Promise<void> {
    platforms.value = await api.listPlatforms();
  }

  async function connect(platform: PlatformId): Promise<void> {
    // 打开平台登录 WebView（用户在其中完成登录）
    const conn = await api.connectPlatform(platform);
    upsertPlatform(conn);
  }

  /** 用户在登录窗口完成登录后确认：探测登录态并落地连接（FR-001/006）。 */
  async function confirmConnection(platform: PlatformId): Promise<void> {
    upsertPlatform(await api.confirmConnection(platform));
  }

  async function refreshStatus(platform: PlatformId): Promise<void> {
    upsertPlatform(await api.getPlatformStatus(platform));
  }

  async function disconnect(platform: PlatformId): Promise<void> {
    await api.disconnectPlatform(platform);
    await refreshPlatforms();
  }

  function upsertPlatform(conn: PlatformConnection): void {
    const i = platforms.value.findIndex((p) => p.platform === conn.platform);
    if (i >= 0) platforms.value[i] = conn;
    else platforms.value.push(conn);
  }

  /** 把文章正文渲染后同步到选定平台（FR-007/013）。 */
  async function syncArticle(
    articlePath: string,
    title: string,
    markdownBody: string,
    targets: PlatformId[],
    digest?: string | null,
    cover?: string | null,
  ): Promise<SyncJob[]> {
    syncing.value = true;
    // 重置本批次结果
    jobs.value = {} as Record<PlatformId, SyncJob>;
    try {
      const renderedHtml = await renderArticleHtml(markdownBody);
      const result = await api.syncArticle({
        articlePath,
        renderedHtml,
        title,
        digest: digest ?? null,
        cover: cover ?? null,
        platforms: targets,
      });
      for (const job of result) {
        jobs.value = { ...jobs.value, [job.platform]: job };
      }
      return result;
    } finally {
      syncing.value = false;
    }
  }

  /** 仅重试某个失败平台（FR-016），新建独立草稿（FR-016a）。 */
  async function retry(
    articlePath: string,
    title: string,
    markdownBody: string,
    platform: PlatformId,
    digest?: string | null,
    cover?: string | null,
  ): Promise<SyncJob> {
    const renderedHtml = await renderArticleHtml(markdownBody);
    const job = await api.retrySync(
      articlePath,
      renderedHtml,
      title,
      digest ?? null,
      cover ?? null,
      platform,
    );
    jobs.value = { ...jobs.value, [job.platform]: job };
    return job;
  }

  async function loadHistory(articlePath: string): Promise<void> {
    history.value = await api.getSyncHistory(articlePath);
  }

  function isConnected(platform: PlatformId): boolean {
    return (
      platforms.value.find((p) => p.platform === platform)?.status ===
      "Connected"
    );
  }

  return {
    platforms,
    jobs,
    syncing,
    history,
    initProgress,
    disposeProgress,
    refreshPlatforms,
    connect,
    confirmConnection,
    refreshStatus,
    disconnect,
    syncArticle,
    retry,
    loadHistory,
    isConnected,
  };
});
