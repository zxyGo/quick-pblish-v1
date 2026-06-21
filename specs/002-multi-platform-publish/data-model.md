# Phase 1 Data Model: 多平台发布

实体来自 spec 的 Key Entities，结合 research.md 的决策细化字段、关系、状态与校验规则。
凡涉及存储位置已注明：会话为加密 blob（OS 密钥）、历史为 SQLite 派生缓存，均非内容真相来源。

## 枚举

### PlatformId
受支持平台标识（MVP 三个，新增平台仅扩此枚举 + 加一个 adapter）。

```
PlatformId = "weixin" | "zhihu" | "juejin"
```

### PlatformStatus
平台连接状态（FR-003 / FR-006）。

```
PlatformStatus = "Disconnected" | "Connected" | "NeedReauth"
```

### SyncStatus
单个同步任务状态（FR-014）。

```
SyncStatus = "Pending" | "Running" | "Success" | "Failed"
```

## 实体

### PlatformConnection（平台连接）
用户与某平台的连接关系。每平台至多一条。

| 字段 | 类型 | 说明 | 校验/规则 |
|------|------|------|-----------|
| platform | PlatformId | 平台标识 | 主键，唯一 |
| status | PlatformStatus | 当前状态 | 由 `probe_login` 实时刷新，不长期信任缓存（FR-006） |
| accountLabel | string \| null | 已连接账号标识（昵称/名称） | Connected 时非空（FR-003） |
| lastCheckedAt | string(ISO8601) \| null | 最近一次登录态校验时间 | — |

**持久化**：`status/accountLabel/lastCheckedAt` 为运行时派生，不落盘为真相；落盘的只有下方会话凭据。

### SessionCredential（会话凭据，加密）
某平台可复用登录态的持久化载体（FR-002 / FR-005）。**不出现在前端类型中**，仅后端 `publish/session.rs` 内部使用。

| 字段 | 类型 | 说明 | 规则 |
|------|------|------|------|
| platform | PlatformId | 所属平台 | — |
| ciphertext | bytes | AES-GCM 加密后的会话 blob（Cookie 等） | 落盘 app data；MUST NOT 明文 |
| nonce | bytes | AES-GCM nonce | 每次加密随机生成 |
| keyRef | string | OS keyring 中加密密钥的条目名 | 密钥本体存 OS 安全设施，不落盘 |

**生命周期**：登录成功→提取+加密+写盘；启动→解密+回灌 WebView；断开（FR-004）→删密文 + 删 keyring 密钥 + 清 WebView 数据目录。

### PublishAdapter（平台适配器，行为契约 / 非持久实体）
每平台一个，互不依赖（章程原则 II）。对外提供统一能力（在 `adapters/mod.rs` 定义 trait）：

| 能力 | 说明 |
|------|------|
| `id() -> PlatformId` | 平台标识 |
| `login_url() -> str` | 登录页 URL（供 WebView 打开） |
| `probe_login_js() -> str` | 探测登录态/账号标识的注入 JS（R4） |
| `transform_html(base_html) -> str` | 把 doocs/md 渲染的 HTML 平台化（FR-011） |
| `upload_image_js(image_b64, meta) -> str` | 复用会话上传单图、返回平台 URL 的注入 JS（R6） |
| `save_draft_js(title, html) -> str` | 复用会话新建草稿、返回草稿引用的注入 JS（R7） |
| `map_error(raw) -> AppError` | 平台原始错误 → 统一错误（含 Auth/Network/Platform） |

测试用 `MockAdapter` 实现同一 trait，用于 `sync.rs` 编排单测（不触网）。

### SyncJob（同步任务）
"把某文章同步到某平台"的一次执行单元（FR-007/013/015/016）。批量同步 = 多个 SyncJob。

| 字段 | 类型 | 说明 | 规则 |
|------|------|------|------|
| id | string | 任务 id | 批次内唯一 |
| articlePath | string | 来源文章相对路径 | 必须存在于当前工作目录 |
| platform | PlatformId | 目标平台 | 对应平台须 Connected，否则前端拦截（FR-012） |
| status | SyncStatus | 状态 | Pending→Running→Success/Failed |
| failureReason | string \| null | 失败原因（可读） | Failed 时非空（FR-014/SC-007） |
| draftRef | DraftRef \| null | 成功时平台草稿引用 | Success 时非空 |
| startedAt / finishedAt | string(ISO8601) \| null | 起止时间 | — |

**状态转移**：`Pending → Running → (Success | Failed)`；Failed 可经重试回到 `Running`（FR-016）。每次执行均新建平台草稿（FR-016a），不复用 draftRef 去覆盖。

### DraftRef（草稿引用）
| 字段 | 类型 | 说明 |
|------|------|------|
| platform | PlatformId | 平台 |
| draftId | string \| null | 平台草稿 id（若可得） |
| url | string \| null | 草稿可访问链接（"前往平台查看"） |

### SyncRecord（同步历史记录）
SyncJob 完成后的留存条目（FR-018）。**SQLite 派生缓存**表 `sync_record`，可清空/重建。

| 字段 | 类型 | 说明 |
|------|------|------|
| id | integer | 自增主键 |
| articlePath | string | 文章相对路径 |
| platform | PlatformId | 目标平台 |
| status | SyncStatus | 终态（Success/Failed） |
| failureReason | string \| null | 失败原因 |
| draftUrl | string \| null | 草稿链接 |
| syncedAt | string(ISO8601) | 同步时间 |

索引：`(articlePath, syncedAt desc)` 支撑"按文章查历史"（US4）。

## 关系图（文字）

```
PlatformConnection 1—1 SessionCredential（加密落盘，密钥在 OS keyring）
PlatformId        1—1 PublishAdapter（注册表静态绑定）
文章(001) 1—N SyncJob N—1 PlatformId
SyncJob   1—0..1 DraftRef
SyncJob   1—1 SyncRecord（完成后留存到派生缓存）
```

## 校验规则汇总

- 同步前：目标平台必须 `Connected`，否则拦截并提示登录（FR-012）。
- 图片：HTML 中所有本地 `assets/` 引用必须在草稿中替换为平台 URL；任一失败 → 整个 SyncJob `Failed`（FR-010a/SC-005）。
- 草稿：只调保存草稿接口，禁止发布；每次新建（FR-008/FR-016a）。
- 凭据：`SessionCredential.ciphertext` 必须加密，密钥仅存 OS keyring（FR-005）。
- 历史：写 `sync_record` 失败不得影响同步主流程成败判定（派生缓存非关键路径）。
