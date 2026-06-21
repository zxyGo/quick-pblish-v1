# Contract: Platform Connection Commands

前后端契约（Tauri command）。类型在 `src/bindings/`（手写，与 Rust 类型保持同步）定义，前端只经 bindings 调用。
下列伪签名描述输入/输出与错误。满足需求：FR-001、FR-002、FR-003、FR-004、FR-006、FR-012。

## 类型

```ts
type PlatformId = "weixin" | "zhihu" | "juejin";

type PlatformStatus = "Disconnected" | "Connected" | "NeedReauth";

type PlatformConnection = {
  platform: PlatformId;
  status: PlatformStatus;
  accountLabel: string | null;   // Connected 时为账号标识
  lastCheckedAt: string | null;  // ISO8601
};

// 沿用并扩充 001 的 AppError
type AppError =
  | { kind: "NotFound";   message: string }
  | { kind: "Permission"; message: string }
  | { kind: "Io";         message: string }
  | { kind: "Conflict";   message: string }
  | { kind: "Invalid";    message: string }
  | { kind: "Auth";       message: string }   // 新增：未登录/登录态失效
  | { kind: "Network";    message: string }   // 新增：网络失败
  | { kind: "Platform";   message: string };  // 新增：平台写入/改版导致的失败
```

## Commands

### `list_platforms() -> Result<PlatformConnection[], AppError>`

- 返回全部受支持平台及其当前连接状态（含未连接的）。供 `PlatformPanel` 渲染（FR-003）。

### `connect_platform(platform: PlatformId) -> Result<PlatformConnection, AppError>`

- 打开该平台的内嵌登录 WebView，等待用户完成登录；成功后提取并加密保存会话（FR-001、FR-002、FR-005）。
- 返回连接结果（`Connected` + accountLabel）。
- **登录生命周期语义**：用户主动关闭登录窗口、放弃登录、或超过登录等待超时（默认 5 分钟），均判为 `Auth`（message 区分"已取消/超时"），不留下半连接状态；登录过程中的验证码/多步验证由用户在 WebView 内自行完成，不视为失败。
- 前端 MUST 允许用户取消正在进行的连接（取消即映射为 `Auth`）。

### `get_platform_status(platform: PlatformId) -> Result<PlatformConnection, AppError>`

- 实时探测该平台登录态（注入 `probe_login_js`），刷新 `status/accountLabel/lastCheckedAt`（FR-006）。
- 过期 → 返回 `NeedReauth`，绝不把过期误报为 `Connected`。

### `disconnect_platform(platform: PlatformId) -> Result<void, AppError>`

- 清除该平台会话：删加密 blob + 删 OS keyring 密钥 + 清 WebView 数据目录（FR-004）。
- 之后该平台回到 `Disconnected`。

## 验收映射

- FR-001/002 → `connect_platform` 成功后 `list_platforms` 显示 `Connected`，重启后仍为 `Connected`（会话回灌）。
- FR-003 → `list_platforms`/`get_platform_status` 返回状态与 accountLabel。
- FR-004 → `disconnect_platform` 后状态 `Disconnected` 且本地无残留凭据。
- FR-006 → 登录态过期时 `get_platform_status` 返回 `NeedReauth`。
- FR-012 → 双层生效：前端依据 status 预拦截对未连接平台的同步（UX）；后端在 `sync_article`/`retry_sync` 执行前再防御性校验，失败则该平台 SyncJob 判 `Failed`(reason=`Auth`)。
- 登录生命周期 → `connect_platform` 在窗口关闭/超时/取消时返回 `Auth`，不产生半连接状态。
