// 前后端共享类型契约。
// 注意：当前为手写绑定（tauri-specta 稳定后改为自动生成）。
// 这些类型与 src-tauri/src/domain.rs 一一对应，由 Rust 端契约测试守护 JSON 形状。
// 详见 specs/001-local-content-management/research.md 第 2 节。

export interface Workspace {
  path: string;
  name: string;
  lastOpened: string;
}

export interface ArticleSummary {
  relativePath: string;
  title: string;
  tags: string[];
  created: string;
  updated: string;
  excerpt: string;
}

export interface ArticleContent {
  relativePath: string;
  title: string;
  tags: string[];
  created: string;
  updated: string;
  body: string;
  baseHash: string;
}

export type NodeKind = "file" | "directory";

export interface FileNode {
  relativePath: string;
  name: string;
  kind: NodeKind;
  isArticle: boolean;
  children: FileNode[];
}

export interface ImportedAsset {
  relativePath: string;
  fileName: string;
}

export interface IndexStatus {
  total: number;
  rebuilding: boolean;
}

export type ConflictStrategy = "abort" | "overwrite" | "saveAs";

export interface SaveArticleInput {
  relativePath: string;
  title: string;
  tags: string[];
  body: string;
  baseHash: string;
  onConflict?: ConflictStrategy;
  saveAsPath?: string;
}

export interface ListQuery {
  keyword?: string;
  sortBy?: "updated" | "created" | "title";
  order?: "asc" | "desc";
}

export type AppErrorKind =
  | "NotFound"
  | "Permission"
  | "Io"
  | "Conflict"
  | "Invalid"
  // 002-multi-platform-publish
  | "Auth"
  | "Network"
  | "Platform";

export interface AppError {
  kind: AppErrorKind;
  message: string;
}

// ===== 002-multi-platform-publish（contracts/platform.md、contracts/publish.md） =====

export type PlatformId = "weixin" | "zhihu" | "juejin";

export type PlatformStatus = "Disconnected" | "Connected" | "NeedReauth";

export interface PlatformConnection {
  platform: PlatformId;
  status: PlatformStatus;
  accountLabel: string | null;
  lastCheckedAt: string | null;
}

export type SyncStatus = "Pending" | "Running" | "Success" | "Failed";

export interface DraftRef {
  platform: PlatformId;
  draftId: string | null;
  url: string | null;
}

export interface SyncJob {
  id: string;
  articlePath: string;
  platform: PlatformId;
  status: SyncStatus;
  failureReason: string | null;
  draftRef: DraftRef | null;
  startedAt: string | null;
  finishedAt: string | null;
}

export interface SyncRequest {
  articlePath: string;
  renderedHtml: string;
  /** 文章 Markdown 正文（与 renderedHtml 同源）；知乎/掘金走编辑器 UI 自动化时以此为内容源。 */
  markdown: string;
  title: string;
  /** 文章摘要；留空时后端从正文文本自动兜底提取。 */
  digest?: string | null;
  /** 封面图片引用（本地相对路径或 URL）；留空时取正文首图兜底。 */
  cover?: string | null;
  platforms: PlatformId[];
}

export interface SyncRecord {
  id: number;
  articlePath: string;
  platform: PlatformId;
  status: SyncStatus;
  failureReason: string | null;
  draftUrl: string | null;
  syncedAt: string;
}

/** 平台展示名（UI 用）。 */
export const PLATFORM_LABELS: Record<PlatformId, string> = {
  weixin: "微信公众号",
  zhihu: "知乎",
  juejin: "掘金",
};
