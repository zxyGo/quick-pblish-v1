# Contract: Article Commands

满足需求：FR-004~009、FR-015~020。类型经 tauri-specta 生成。

## 类型

```ts
type ArticleSummary = {
  relativePath: string;
  title: string;
  tags: string[];
  created: string;    // ISO8601
  updated: string;    // ISO8601
  excerpt: string;
};

type ArticleContent = {
  relativePath: string;
  title: string;
  tags: string[];
  created: string;
  updated: string;
  body: string;         // front matter 之后的正文
  baseHash: string;     // 打开时的正文哈希，保存时回传用于冲突检测
};

type SaveArticleInput = {
  relativePath: string;
  title: string;
  tags: string[];
  body: string;
  baseHash: string;     // 打开时拿到的哈希
  onConflict?: "abort" | "overwrite" | "saveAs";  // 默认 abort
  saveAsPath?: string;  // onConflict = saveAs 时使用
};

type ListQuery = {
  keyword?: string;                 // 标题/标签/正文检索（FR-017）
  sortBy?: "updated" | "created" | "title";
  order?: "asc" | "desc";
};
```

## Commands

### `list_articles(query: ListQuery) -> Result<ArticleSummary[], AppError>`

- 从派生缓存返回文章摘要列表，支持检索与排序（FR-015、FR-017）。
- 元数据缺失/损坏的文章以默认值降级返回，不中断（FR-018）。

### `read_article(relativePath: string) -> Result<ArticleContent, AppError>`

- 读取并解析 front matter + 正文，返回内容与 `baseHash`（FR-005、用于 FR-019）。

### `create_article(input: { relativePath: string; title?: string }) -> Result<ArticleContent, AppError>`

- 在工作目录创建新 `.md`（写入初始 front matter）。文件名冲突 → `Conflict`（FR-020、FR-004）。

### `save_article(input: SaveArticleInput) -> Result<ArticleContent, AppError>`

- 写入 front matter + 正文；保存前用 `baseHash` 与磁盘当前哈希比对：
  - 一致 → 正常保存，更新 `updated`，刷新缓存（FR-005、FR-008）。
  - 不一致且 `onConflict = abort` → 返回 `Conflict`（前端弹窗，FR-019）。
  - `overwrite` → 覆盖保存；`saveAs` → 写入 `saveAsPath` 新文件。

### `delete_article(relativePath: string) -> Result<void, AppError>`

- 将文章文件移入系统回收站（FR-006/Q3）。确认由前端负责。

### `update_metadata(input: { relativePath: string; title?: string; tags?: string[] }) -> Result<ArticleSummary, AppError>`

- 仅更新 front matter 的 title/tags 并刷新缓存（FR-016）。

## 验收映射

- FR-004 → `create_article` 产出 `.md`。
- FR-005 → `read_article`/`save_article` 往返一致。
- FR-006 → `delete_article` 入回收站。
- FR-007 → 未保存提示由前端依据 Dirty 状态处理（见 data-model 状态流）。
- FR-016 → `update_metadata` 写 front matter。
- FR-017 → `list_articles` 的 keyword/sort。
- FR-018 → 损坏元数据降级返回。
- FR-019 → `save_article` 的 baseHash 冲突分支。
- FR-020 → `create_article` 文件名冲突 `Conflict`。
