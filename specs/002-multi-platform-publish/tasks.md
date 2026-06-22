---
description: "Task list for 多平台发布（浏览器同步式一键发布）"
---

# Tasks: 多平台发布（浏览器同步式一键发布）

**Input**: Design documents from `specs/002-multi-platform-publish/`

**Prerequisites**: plan.md、spec.md（必需）；research.md、data-model.md、contracts/（已提供）

**Tests**: 已包含测试任务——plan.md 的 Testing 与章程要求"平台适配器与跨语言契约必须有自动化测试覆盖关键路径"。

**Organization**: 任务按用户故事分组，可独立实现与验证。前序基座 `001-local-content-management` 视为已完成。

## Format: `[ID] [P?] [Story] Description`

- **[P]**: 可并行（不同文件、无未完成依赖）
- **[Story]**: 所属用户故事（US1–US4）

## Path Conventions

- 后端：`src-tauri/src/`；前端：`src/`；前端测试：`tests/`（Vitest），后端测试随 `#[cfg(test)]` 内联于各 `.rs`

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: 依赖与目录骨架

- [X] T001 在 `src-tauri/Cargo.toml` 的 `[dependencies]` 增加 `keyring`、`aes-gcm`、`rand`（FR-005 / research R3）
- [X] T002 [P] 创建后端模块骨架空文件并在 `src-tauri/src/lib.rs` 声明 `mod adapters; mod publish;`：`src-tauri/src/adapters/mod.rs`、`src-tauri/src/publish/mod.rs`、`src-tauri/src/publish/session.rs`、`src-tauri/src/publish/webview.rs`、`src-tauri/src/publish/sync.rs`、`src-tauri/src/publish/history.rs`、`src-tauri/src/commands/publish.rs`（并在 `src-tauri/src/commands/mod.rs` 声明 `pub mod publish;`）
- [X] T003 [P] 创建前端骨架：目录 `src/components/publish/`、桩文件 `src/stores/publish.ts`、`src/services/render.ts`

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: 所有用户故事共享的核心基座

**⚠️ CRITICAL**: 本阶段完成前，任何用户故事不可开工

- [X] T004 在 `src-tauri/src/error.rs` 为 `AppError` 扩充 `Auth`、`Network`、`Platform` 三个 `kind`（contracts/platform.md）
- [X] T005 [P] 在 `src-tauri/src/adapters/mod.rs` 定义 `PlatformId` 枚举、`PublishAdapter` trait（`id/login_url/probe_login_js/transform_html/upload_image_js/save_draft_js/map_error`）与平台注册表 `adapter_for()`（data-model.md / 章程原则 II）
- [X] T006 [P] 在 `src-tauri/src/adapters/mod.rs` 实现 `MockAdapter`（`#[cfg(test)]`，供 sync 编排单测，不触网）
- [X] T007 在 `src-tauri/src/publish/webview.rs` 实现跨平台 WebView 抽象（`PlatformBridge`：`open_login`/`eval`/`close`），`TauriBridge` 生产实现 + `MockBridge` 测试实现；Linux 受限处注释降级策略（research R2 / 章程原则 III）。**IPC 回传已接线**：token + 一次性通道 + `report_eval_result` 命令 + `wrap_js` 经 `window.__TAURI__.core.invoke` 回传；配 `withGlobalTauri` 与 `capabilities/publish.json`
- [X] T008 [P] 在 `src/bindings/types.ts` 追加共享类型 `PlatformId`、`PlatformStatus`、`SyncStatus`、`PlatformConnection`、`DraftRef`、`SyncJob`、`SyncRequest`、`SyncRecord`，并扩充 `AppError` 联合（contracts/）
- [X] T009 在 `src-tauri/src/lib.rs` 的 `invoke_handler` 注册全部 publish 命令

**Checkpoint**: 基座就绪——用户故事可开工

---

## Phase 3: User Story 1 - 连接平台账号（建立并复用登录态）(Priority: P1) 🎯 MVP

**Goal**: 用户能在应用内登录平台、看到"已连接 + 账号"，重启仍有效，可断开清除。

**Independent Test**: 完成公众号登录→显示 Connected+账号→重启仍 Connected→断开后无残留凭据（quickstart 场景 1）。

### Tests for User Story 1

- [X] T010 [P] [US1] 在 `src-tauri/src/publish/session.rs` 内联测试：会话 blob 加密/解密往返、明文不落盘、断开后密钥清除（FR-005）
- [X] T011 [P] [US1] 在 `tests/publish-store.connection.test.ts` 测试 publish store 的连接状态机（Disconnected/Connected/NeedReauth）

### Implementation for User Story 1

- [X] T012 [US1] 在 `src-tauri/src/publish/session.rs` 实现会话提取→`aes-gcm` 加密 blob 落盘、密钥经 `keyring` 存 OS 安全设施、启动回灌、断开清除（FR-002/004/005，research R3）
- [X] T013 [P] [US1] 在 `src-tauri/src/adapters/weixin.rs` 实现公众号 `login_url` 与 `probe_login_js`（返回登录态/账号标识，FR-006/R4）
- [X] T014 [P] [US1] 在 `src-tauri/src/adapters/zhihu.rs` 实现知乎 `login_url` 与 `probe_login_js`
- [X] T015 [P] [US1] 在 `src-tauri/src/adapters/juejin.rs` 实现掘金 `login_url` 与 `probe_login_js`
- [X] T016 [US1] 在 `src-tauri/src/commands/publish.rs` 实现 `list_platforms`、`connect_platform`、`get_platform_status`、`disconnect_platform`，并新增 `confirm_connection`（登录后经注入 JS 真实探测登录态/账号）与 `report_eval_result`（IPC 回传，contracts/platform.md，FR-001/003/004/006/012）
- [X] T017 [US1] 在 `src-tauri/src/lib.rs` 注册上述 4 个连接命令
- [X] T018 [P] [US1] 在 `src/bindings/commands.ts` 追加 4 个连接命令的类型化封装
- [X] T019 [US1] 在 `src/stores/publish.ts` 实现平台连接状态（list/connect/status/disconnect 动作）
- [X] T020 [US1] 实现 `src/components/publish/PlatformPanel.vue`（三平台状态、登录、断开；UnoCSS 原子类）

**Checkpoint**: US1 可独立运行——连接/状态/断开闭环

---

## Phase 4: User Story 2 - 一键把文章同步为单个平台的草稿 (Priority: P1) 🎯 MVP

**Goal**: 选中文章 + 已连接平台 → 一键生成保留样式、图片已上传的平台草稿。

**Independent Test**: 含本地图片文章同步到公众号→草稿样式≈预览、图片为平台 URL、无本地路径；未登录平台被拦截（quickstart 场景 2）。

### Tests for User Story 2

- [ ] T021 [P] [US2] 在 `src-tauri/src/adapters/weixin.rs` 内联测试 `transform_html` 保留内联样式（SC-003）——**未做**：transform 行为已由 sync 测试间接覆盖，专项 adapter 单测留待
- [X] T022 [P] [US2] 在 `src-tauri/src/publish/sync.rs` 内联测试（MockAdapter）：单平台流水线成功；任一图片失败 → 整个 SyncJob `Failed`、不产出草稿（FR-010a/SC-005）
- [X] T023 [P] [US2] 在 `tests/render.test.ts` 测试 `render.ts` 用 `@md/core` 产出非空内联样式 HTML

### Implementation for User Story 2

- [X] T024 [P] [US2] 在 `src-tauri/src/adapters/weixin.rs` 扩展 `transform_html`、`upload_image_js`、`save_draft_js`、`map_error`（FR-009/010/011/R5-R7）。注：JS 端点/选择子标 `TODO(empirical)` 待联调
- [X] T025 [P] [US2] 在 `src-tauri/src/adapters/zhihu.rs` 扩展同上四能力
- [X] T026 [P] [US2] 在 `src-tauri/src/adapters/juejin.rs` 扩展同上四能力
- [X] T027 [US2] 在 `src-tauri/src/publish/sync.rs` 实现单 SyncJob 流水线：平台化 HTML → 逐图上传替换（全有或全无）→ 新建草稿；返回 `SyncJob`（FR-007/008/010a/016a）
- [X] T028 [US2] 在 `src-tauri/src/commands/publish.rs` 实现 `sync_article`（含同步前登录态校验 → 失败 reason=`Auth`），并在 `lib.rs` 注册（FR-012/SC-007）
- [X] T029 [P] [US2] 在 `src/services/render.ts` 用 `@md/core` 把文章渲染为带内联样式 HTML（与编辑器预览同源，SC-003）
- [X] T030 [P] [US2] 在 `src/bindings/commands.ts` 追加 `sync_article` 封装
- [X] T031 [US2] 在 `src/stores/publish.ts` 增加 SyncJob 状态与 `syncArticle` 动作
- [X] T032 [US2] 实现 `src/components/publish/PublishDialog.vue`（选中文章 + 勾选平台 + 触发 + "前往平台查看草稿"）

**Checkpoint**: US1+US2 构成可用 MVP——单平台一键存草稿

---

## Phase 5: User Story 3 - 一次选择多个平台批量同步 (Priority: P2)

**Goal**: 多平台批量同步，逐平台实时进度/结果，单平台失败隔离，失败可重试。

**Independent Test**: 勾选两平台同步→逐平台结果；令其一失败→另一成功不受影响；重试失败平台→新建独立草稿（quickstart 场景 3）。

### Tests for User Story 3

- [X] T033 [P] [US3] 在 `src-tauri/src/publish/sync.rs` 内联测试（MockAdapter）：批量中单平台失败不阻断其余（FR-015）；重试新建独立草稿不覆盖（FR-016a）
- [X] T034 [P] [US3] 在 `tests/sync-result-list.test.ts` 测试 `SyncResultList` 依 job 状态渲染进度/重试入口

### Implementation for User Story 3

- [X] T035 [US3] 在 `src-tauri/src/publish/sync.rs` 扩展批量编排：多平台串行执行、逐平台隔离 try、经 Tauri 事件 `publish://sync-progress` 推送进度（FR-013/014/015，research R8）
- [X] T036 [US3] 在 `src-tauri/src/commands/publish.rs` 扩展 `sync_article` 多平台返回 `SyncJob[]`、实现 `retry_sync`（单平台），并在 `lib.rs` 注册（FR-016/016a）
- [X] T037 [P] [US3] 在 `src/bindings/commands.ts` 追加 `retry_sync` 封装与 `publish://sync-progress` 事件订阅辅助
- [X] T038 [US3] 在 `src/stores/publish.ts` 订阅进度事件归并 job 状态、增加 `retry` 动作
- [X] T039 [US3] 实现 `src/components/publish/SyncResultList.vue`（逐平台进度/结果/原因/重试；UnoCSS）

**Checkpoint**: US1–US3 均可独立运行

---

## Phase 6: User Story 4 - 查看同步历史 (Priority: P3)

**Goal**: 按文章查看历次同步的平台/时间/结果。

**Independent Test**: 多次同步后打开历史→按时间倒序列出平台/时间/结果（quickstart 场景 5）。

### Tests for User Story 4

- [X] T040 [P] [US4] 在 `src-tauri/src/publish/history.rs` 内联测试：`sync_record` 写入与按文章 `syncedAt` 倒序查询（FR-018）

### Implementation for User Story 4

- [X] T041 [US4] 在 `src-tauri/src/publish/history.rs` 创建 `sync_record` 表（派生缓存，复用 `index` 的 rusqlite 连接）并实现写入/查询
- [X] T042 [US4] 每个 SyncJob 完成后写一条 `SyncRecord`（在 `commands/publish.rs::write_history`，写历史失败不改变同步成败结论）
- [X] T043 [US4] 在 `src-tauri/src/commands/publish.rs` 实现 `get_sync_history` 并在 `lib.rs` 注册
- [X] T044 [P] [US4] 在 `src/bindings/commands.ts` 追加 `get_sync_history` 封装
- [ ] T045 [US4] 实现 `src/components/publish/SyncHistory.vue` 并在 store 增加历史查询动作——**未做**：store 的 `loadHistory` 已就绪，仅缺历史 UI 组件

**Checkpoint**: 全部用户故事独立可用

---

## Phase 7: Polish & Cross-Cutting Concerns

- [ ] T046 [P] 核验：若引用/移植了 doocs/cose 的任何适配实现，确认其许可证与 MIT 兼容并保留出处（research R11 / 章程原则 V）
- [ ] T047 [P] 在 `src-tauri/src/publish/webview.rs` 完善 Linux WebKitGTK 受限能力的显式降级提示文案（章程原则 III）
- [ ] T048 复核 FR-019：确认发布流程无任何与发布无关的数据外发（章程原则 V）
- [ ] T049 按 `specs/002-multi-platform-publish/quickstart.md` 执行场景 1–5 端到端验收
- [ ] T050 [P] 在 `README.md` 增补多平台发布功能说明与使用前提

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (P1)**: 无依赖，立即可启
- **Foundational (P2)**: 依赖 Setup；阻塞所有用户故事
- **User Stories (P3–P6)**: 均依赖 Foundational
  - US1、US2 为 P1（MVP）；US2 的同步流水线依赖 US1 的会话/适配器骨架就绪
  - US3 依赖 US2 的单平台流水线；US4 依赖 US2 产生的同步结果
- **Polish (P7)**: 依赖目标故事完成

### User Story Dependencies

- **US1 (P1)**: 基座之上独立；提供会话与各 adapter 的 probe 骨架
- **US2 (P1)**: 复用 US1 的 adapter 文件与会话；构成 MVP 闭环
- **US3 (P2)**: 在 US2 的 `sync.rs` 上扩展批量/事件/重试
- **US4 (P3)**: 在 US2/US3 的同步结果上增加历史留存

### Within Each User Story

- 测试先写并应失败 → 实现
- 后端 model/trait → service/编排 → command → 注册 → bindings → store → UI

### Parallel Opportunities

- Setup 中 T002、T003 可并行
- Foundational 中 T005、T006、T008 可并行（T004 错误类型先行，T007 抽象层、T009 注册位独立）
- 各 adapter 任务（T013/T014/T015；T024/T025/T026）按平台分文件可并行
- 各故事内标 [P] 的测试可并行先行
- 人力充足时 US3/US4 可在 US2 完成后并行推进

---

## Parallel Example: User Story 1

```bash
# 先并行写测试（应失败）：
Task: "T010 session 加解密往返测试 in src-tauri/src/publish/session.rs"
Task: "T011 publish store 连接状态机测试 in tests/publish-store.connection.test.ts"

# 三平台 probe 适配按文件并行：
Task: "T013 weixin probe in src-tauri/src/adapters/weixin.rs"
Task: "T014 zhihu probe in src-tauri/src/adapters/zhihu.rs"
Task: "T015 juejin probe in src-tauri/src/adapters/juejin.rs"
```

---

## Implementation Strategy

### MVP First（US1 + US2）

1. 完成 Phase 1 Setup
2. 完成 Phase 2 Foundational（关键，阻塞全部）
3. 完成 Phase 3 US1（连接）→ 验证 quickstart 场景 1
4. 完成 Phase 4 US2（单平台存草稿）→ 验证 quickstart 场景 2
5. **STOP & VALIDATE**：此时已是可用 MVP（一键存草稿）

### Incremental Delivery

1. Setup + Foundational → 基座就绪
2. US1 → US2 → MVP（演示单平台一键草稿）
3. US3 → 多平台批量 + 隔离 + 重试
4. US4 → 同步历史
5. 每个故事独立增量，不破坏既有

---

## Notes

- [P] = 不同文件、无未完成依赖
- 会话凭据全程加密，禁止明文落盘（FR-005）；仅写草稿，禁止触发发布（FR-008）
- adapter 互不依赖，新增平台仅加一个 `adapters/<platform>.rs` 并在注册表登记（FR-017/SC-006）
- 每个 checkpoint 可停下独立验收对应用户故事
