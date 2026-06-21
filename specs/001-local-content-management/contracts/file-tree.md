# Contract: File Tree Commands

满足需求：FR-012、FR-013、FR-014、FR-020。类型经 tauri-specta 生成。

## 类型

```ts
type FileNode = {
  relativePath: string;
  name: string;
  kind: "file" | "directory";
  isArticle: boolean;     // .md 文件
  children: FileNode[];   // directory 时填充
};
```

## Commands

### `get_file_tree() -> Result<FileNode, AppError>`

- 返回当前工作目录的文件树根节点，结构与磁盘一致，包含非 Markdown 素材（FR-012、FR-014）。

### `create_folder(parentRelativePath: string, name: string) -> Result<FileNode, AppError>`

- 新建文件夹。重名 → `Conflict`（FR-013）。

### `rename_path(relativePath: string, newName: string) -> Result<FileNode, AppError>`

- 重命名文件/文件夹。目标已存在 → `Conflict`（FR-013、FR-020，禁止静默覆盖）。

### `move_path(relativePath: string, targetDirRelativePath: string) -> Result<FileNode, AppError>`

- 移动文件/文件夹到目标目录。目标存在同名 → `Conflict`（FR-013）。

### `delete_path(relativePath: string) -> Result<void, AppError>`

- 将文件/文件夹移入系统回收站（FR-013，复用回收站策略）。确认由前端负责。

## 事件（后端 → 前端）

### `workspace_changed`（notify 监听派生）

- 当工作目录内文件发生外部变更时广播，前端据此刷新文件树/列表（支撑 FR-019 的一致性体验）。
- payload：变更类型与受影响相对路径。

## 验收映射

- FR-012 → `get_file_tree` 结构与磁盘一致。
- FR-013 → create/rename/move/delete 同步磁盘。
- FR-014 → 文件树含素材文件。
- FR-020 → rename/move 同名冲突返回 `Conflict`。
