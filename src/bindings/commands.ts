// 对 Tauri command 的强类型封装层。前端只通过这里调用后端，杜绝裸 invoke 字符串。
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  ArticleContent,
  ArticleSummary,
  FileNode,
  ImportedAsset,
  IndexStatus,
  ListQuery,
  PlatformConnection,
  PlatformId,
  SaveArticleInput,
  SyncJob,
  SyncRecord,
  SyncRequest,
  Workspace,
} from "./types";

/** 同步进度事件名（contracts/publish.md）。 */
export const SYNC_PROGRESS_EVENT = "publish://sync-progress";

export const api = {
  // workspace
  selectWorkspace: (path: string) =>
    invoke<Workspace>("select_workspace", { path }),
  switchWorkspace: (path: string) =>
    invoke<Workspace>("switch_workspace", { path }),
  getCurrentWorkspace: () =>
    invoke<Workspace | null>("get_current_workspace"),
  listRecentWorkspaces: () =>
    invoke<Workspace[]>("list_recent_workspaces"),

  // article
  listArticles: (query: ListQuery = {}) =>
    invoke<ArticleSummary[]>("list_articles", { query }),
  readArticle: (relativePath: string) =>
    invoke<ArticleContent>("read_article", { relativePath }),
  createArticle: (relativePath: string, title?: string) =>
    invoke<ArticleContent>("create_article", {
      input: { relativePath, title },
    }),
  saveArticle: (input: SaveArticleInput) =>
    invoke<ArticleContent>("save_article", { input }),
  deleteArticle: (relativePath: string) =>
    invoke<void>("delete_article", { relativePath }),
  updateMetadata: (
    relativePath: string,
    title?: string,
    tags?: string[],
  ) =>
    invoke<ArticleSummary>("update_metadata", {
      input: { relativePath, title, tags },
    }),

  // file tree
  getFileTree: () => invoke<FileNode>("get_file_tree"),
  createFolder: (parentRelativePath: string, name: string) =>
    invoke<FileNode>("create_folder", { parentRelativePath, name }),
  renamePath: (relativePath: string, newName: string) =>
    invoke<FileNode>("rename_path", { relativePath, newName }),
  movePath: (relativePath: string, targetDirRelativePath: string) =>
    invoke<FileNode>("move_path", { relativePath, targetDirRelativePath }),
  deletePath: (relativePath: string) =>
    invoke<void>("delete_path", { relativePath }),

  // asset & index
  importAsset: (sourcePath: string) =>
    invoke<ImportedAsset>("import_asset", { sourcePath }),
  /** 读取工作目录内本地图片为 base64 data URL，供预览渲染本地相对路径图片。 */
  readAssetDataUrl: (relPath: string) =>
    invoke<string>("read_asset_data_url", { relPath }),
  rebuildIndex: () => invoke<IndexStatus>("rebuild_index"),
  getIndexStatus: () => invoke<IndexStatus>("get_index_status"),

  // 002-multi-platform-publish：平台连接
  listPlatforms: () => invoke<PlatformConnection[]>("list_platforms"),
  connectPlatform: (platform: PlatformId) =>
    invoke<PlatformConnection>("connect_platform", { platform }),
  confirmConnection: (platform: PlatformId) =>
    invoke<PlatformConnection>("confirm_connection", { platform }),
  getPlatformStatus: (platform: PlatformId) =>
    invoke<PlatformConnection>("get_platform_status", { platform }),
  disconnectPlatform: (platform: PlatformId) =>
    invoke<void>("disconnect_platform", { platform }),

  // 002-multi-platform-publish：同步 / 重试 / 历史
  syncArticle: (request: SyncRequest) =>
    invoke<SyncJob[]>("sync_article", { request }),
  retrySync: (
    articlePath: string,
    renderedHtml: string,
    title: string,
    platform: PlatformId,
  ) =>
    invoke<SyncJob>("retry_sync", {
      articlePath,
      renderedHtml,
      title,
      platform,
    }),
  getSyncHistory: (articlePath: string) =>
    invoke<SyncRecord[]>("get_sync_history", { articlePath }),
};

/** 订阅同步进度事件（FR-014）。返回取消订阅函数。 */
export function onSyncProgress(
  handler: (job: SyncJob) => void,
): Promise<UnlistenFn> {
  return listen<SyncJob>(SYNC_PROGRESS_EVENT, (e) => handler(e.payload));
}
