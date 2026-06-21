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
  | "Invalid";

export interface AppError {
  kind: AppErrorKind;
  message: string;
}
