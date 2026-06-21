# Contract: Asset & Index Commands

满足需求：FR-014a、FR-008a。类型经 tauri-specta 生成。

## 类型

```ts
type ImportedAsset = {
  relativePath: string;   // 通常 assets/...，相对工作目录
  fileName: string;
};

type IndexStatus = {
  total: number;          // 已索引文章数
  rebuilding: boolean;
};
```

## Commands

### `import_asset(sourcePath: string) -> Result<ImportedAsset, AppError>`

- 将外部图片复制进工作目录 `assets/`（必要时去重命名避免覆盖），返回相对路径供正文引用（FR-014a/Q4）。
- 前端在编辑器插入图片时调用，把 `relativePath` 写入 Markdown。

### `rebuild_index() -> Result<IndexStatus, AppError>`

- 清空并从工作目录 Markdown 文件全量重建 SQLite 派生缓存（FR-008a）。
- 用于缓存损坏/缺失的恢复，或用户手动触发。绝不影响真相来源文件。

### `get_index_status() -> Result<IndexStatus, AppError>`

- 返回当前索引状态（文章数、是否正在重建），供 UI 展示。

## 验收映射

- FR-014a → `import_asset` 复制进 assets 并返回相对路径。
- FR-008a → `rebuild_index` 可从文件完整重建缓存，缓存非真相来源。
