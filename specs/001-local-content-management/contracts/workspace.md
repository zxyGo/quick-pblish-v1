# Contract: Workspace Commands

前后端契约（Tauri command）。类型由 Rust 端经 tauri-specta 生成 TS 绑定，前端勿手写。
下列用类语言伪签名描述输入/输出与错误，实际类型以 `src/bindings/` 生成结果为准。

满足需求：FR-001、FR-002、FR-003。

## 类型

```ts
type Workspace = {
  path: string;        // 绝对路径
  name: string;
  lastOpened: string;  // ISO8601
};

type AppError =
  | { kind: "NotFound";    message: string }
  | { kind: "Permission";  message: string }
  | { kind: "Io";          message: string }
  | { kind: "Conflict";    message: string }
  | { kind: "Invalid";     message: string };
```

## Commands

### `select_workspace(path: string) -> Result<Workspace, AppError>`

- 将给定目录设为当前工作目录并持久化；更新 recent 列表。
- 错误：路径不存在 → `NotFound`；不可写 → `Permission`。

### `get_current_workspace() -> Result<Workspace | null, AppError>`

- 返回上次持久化的当前工作目录；首次启动返回 `null`（前端引导选择，FR-001）。
- 若持久化目录已不可访问，返回的 Workspace 标记由前端据后续操作的 `NotFound/Permission` 错误进入重选流程（FR-003）。

### `switch_workspace(path: string) -> Result<Workspace, AppError>`

- 切换到另一目录，触发缓存对新目录的（重）同步（FR-002）。

### `list_recent_workspaces() -> Result<Workspace[], AppError>`

- 返回最近使用的工作目录列表。

## 验收映射

- FR-001 → `get_current_workspace` 返回上次目录；首次 `null`。
- FR-002 → `switch_workspace` 后列表/文件树反映新目录。
- FR-003 → 目录不可访问时命令返回 `NotFound`/`Permission`，前端引导重选，禁止崩溃。
