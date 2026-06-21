// 对 Tauri command 的强类型封装层。前端只通过这里调用后端，杜绝裸 invoke 字符串。
import { invoke } from "@tauri-apps/api/core";
import type {
  ArticleContent,
  ArticleSummary,
  FileNode,
  ImportedAsset,
  IndexStatus,
  ListQuery,
  SaveArticleInput,
  Workspace,
} from "./types";

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
  rebuildIndex: () => invoke<IndexStatus>("rebuild_index"),
  getIndexStatus: () => invoke<IndexStatus>("get_index_status"),
};
