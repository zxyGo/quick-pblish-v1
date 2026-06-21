# Contract: Publish / Sync / History Commands

前后端契约（Tauri command）。类型见 `src/bindings/`。满足需求：FR-007~FR-011、FR-013~FR-016a、FR-018。
错误类型沿用 [platform.md](./platform.md) 的 `AppError`（含 `Auth`/`Network`/`Platform`）。

## 类型

```ts
type SyncStatus = "Pending" | "Running" | "Success" | "Failed";

type DraftRef = {
  platform: PlatformId;
  draftId: string | null;
  url: string | null;       // “前往平台查看草稿”
};

type SyncJob = {
  id: string;
  articlePath: string;      // 工作目录相对路径
  platform: PlatformId;
  status: SyncStatus;
  failureReason: string | null;
  draftRef: DraftRef | null;
  startedAt: string | null;
  finishedAt: string | null;
};

// 同步入参：renderedHtml 由前端用 @md/core 渲染（与预览同源，FR-009/SC-003）
type SyncRequest = {
  articlePath: string;
  renderedHtml: string;     // 带内联样式的 HTML
  title: string;
  platforms: PlatformId[];  // 单个或多个（FR-007/FR-013）
};

type SyncRecord = {
  id: number;
  articlePath: string;
  platform: PlatformId;
  status: SyncStatus;       // Success | Failed
  failureReason: string | null;
  draftUrl: string | null;
  syncedAt: string;         // ISO8601
};
```

## Commands

### `sync_article(request: SyncRequest) -> Result<SyncJob[], AppError>`

- 为 `platforms` 中每个平台创建一个 `SyncJob` 并**串行**执行：平台化 HTML → 逐图上传替换 → 新建草稿（R5/R6/R7）。
- 任一平台失败仅标记其 `SyncJob.status = Failed` 并附 `failureReason`，不影响其余平台（FR-015）。
- 同步前对每个目标平台做**后端防御性**登录态校验，未连接/失效 → 该 job 直接 `Failed`（reason=`Auth`），不写草稿、不静默（FR-012/SC-007）。此为前端预拦截之外的第二道闸；二者共同满足 FR-012（详见 [platform.md](./platform.md)）。
- 返回全部 SyncJob 的终态数组；过程中经事件流推送进度（见下）。

### 进度事件（Tauri event，非 command）

- 事件名：`publish://sync-progress`，载荷为单个 `SyncJob` 的最新快照。
- 前端 `SyncResultList` 订阅以实时更新逐平台进度（FR-014）。

### `retry_sync(articlePath: string, renderedHtml: string, title: string, platform: PlatformId) -> Result<SyncJob, AppError>`

- 仅对单个失败平台重试（FR-016）。与 `sync_article` 同样逻辑，新建独立草稿（FR-016a），不覆盖历史草稿。

### `get_sync_history(articlePath: string) -> Result<SyncRecord[], AppError>`

- 返回某文章的同步历史，按 `syncedAt` 倒序（FR-018，US4）。读自 SQLite 派生缓存 `sync_record`。

## 行为约束

- **只写草稿**：所有平台仅调用"保存草稿"路径，MUST NOT 触发发布/群发（FR-008）。
- **图片全有或全无**：任一图片上传失败 → 整个 SyncJob `Failed`，不产出不完整草稿（FR-010a/SC-005）。
- **历史非关键路径**：写 `sync_record` 失败不改变 SyncJob 的成败结论。

## 验收映射

- FR-007/013 → `sync_article` 接受单/多平台，返回逐平台 SyncJob。
- FR-009 → 入参 `renderedHtml` 与预览同源；公众号 adapter 保留内联样式（SC-003）。
- FR-010/010a → 草稿图片替换为平台 URL；任一失败整篇判失败（SC-005）。
- FR-014 → `publish://sync-progress` 事件逐平台推进度/结果。
- FR-015 → 单平台 Failed 不阻断其余 job。
- FR-016/016a → `retry_sync` 仅重试失败平台并新建草稿。
- FR-018 → `get_sync_history` 返回按时间倒序的记录。
