# Implementation Plan: 多平台发布（浏览器同步式一键发布）

**Branch**: `002-multi-platform-publish` | **Date**: 2026-06-21 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `specs/002-multi-platform-publish/spec.md`

## Summary

把 doocs/cose 浏览器扩展的"复用登录态 → 写入平台草稿"能力内建进本桌面应用。技术路线：在 Tauri 内为每个平台
开**独立的内嵌 WebView**，用户在其中按平台原生方式登录，会话由系统 WebView 持有；发布时向该 WebView **注入
JS**，复用其登录态，驱动平台自身的内部接口/DOM 把渲染好的内容写入草稿、上传文中插图。每个平台实现为
`src-tauri/src/adapters/` 下一个独立适配器（首次落地章程原则 II 预留的边界），核心编辑/文件/文章管理代码零改动。
登录态以加密 blob 落盘、密钥存 OS 安全设施。MVP 覆盖微信公众号 + 知乎 + 掘金。

## Technical Context

**Language/Version**: 前端 TypeScript 5.6（Vue 3.5，strict）；后端 Rust 1.75+（Tauri 2.x）

**Primary Dependencies**:
- 前端（已有）：Vue 3、tdesign-vue-next、UnoCSS、Pinia、Vue Router、Vite；`@md/core`（doocs/md 渲染核心，用于把文章渲染为带内联样式的 HTML）
- 后端（已有）：Tauri 2、serde、rusqlite（bundled）、thiserror、chrono、sha2
- 后端（本切片新增）：
  - `keyring` —— 跨平台访问 OS 安全设施（Windows Credential Manager / macOS Keychain / Linux Secret Service），用于保存会话加密密钥（FR-005）
  - `aes-gcm` + `rand` —— 对会话 blob 做对称加密（密钥来自 keyring）
  - 复用 Tauri 多 WebView 能力（`WebviewWindowBuilder` / `webview.eval`），无需额外 crate

**Storage**:
- 真相来源仍是本地 Markdown + `assets/`（由 001 提供）
- 会话凭据：加密 blob 落盘于 app data 目录，密钥存 OS 安全设施（FR-005）
- 同步历史：SQLite 派生缓存新增表 `sync_record`（可重建/可清空，非真相来源）

**Testing**: 后端 `cargo test`（会话加解密、各 adapter 的 HTML 平台化转换、sync 编排与逐平台隔离/重试、history 读写，以 `MockAdapter` 覆盖编排逻辑）；前端 Vitest（publish store、连接面板、同步对话框/结果列表组件）；WebView 内 JS 注入属集成层，以契约 + 手动 quickstart 验证

**Target Platform**: 桌面端 Windows 10+ / macOS 12+ / Linux（WebView2 / WKWebView / WebKitGTK）

**Project Type**: desktop-app（Tauri 单仓库：Vue 前端 + Rust 后端）

**Performance Goals**:
- 已登录、网络正常下，单平台从触发到出现草稿 ≤ 60s（SC-001）
- 公众号草稿样式与预览一致率 ≥ 95%（SC-003）

**Constraints**:
- 仅写草稿，绝不自动公开发布（FR-008）
- 会话凭据禁止明文落盘（FR-005，章程原则 I）
- 三平台 WebView 行为差异必须抽象处理，受限能力显式降级提示（章程原则 III）
- 前后端契约强类型、TS strict、禁止 `any`（章程原则 IV）
- 不收集与发布无关数据、无遥测（FR-019，章程原则 V）

**Scale/Scope**: 单用户单机；本切片 4 个用户故事、~20 条 FR；MVP 3 个平台适配器（公众号/知乎/掘金）

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| 原则 | 要求 | 本计划如何满足 | 状态 |
|------|------|----------------|------|
| I. 本地优先与数据自主 | 本地优先、仅发布可联网、凭据加密 | 真相来源仍是本地 Markdown；仅"同步草稿"联网；会话以 AES-GCM 加密落盘、密钥存 OS keyring（FR-005） | PASS |
| II. 平台适配器解耦 | 平台为独立适配器、核心不改 | 落地 `src-tauri/src/adapters/`：`PublishAdapter` trait + weixin/zhihu/juejin 三实现，互不依赖；新增平台仅加 adapter，编辑/文件/文章管理零改动（FR-017、SC-006） | PASS |
| III. 跨平台一致性 | 三平台一致、受限显式降级、CI 三平台 | WebView 操作经统一抽象层封装 WebView2/WKWebView/WebKitGTK 差异；Linux 能力受限处显式提示而非静默失败；沿用三平台 CI 矩阵 | PASS |
| IV. 端到端类型安全 | 契约强类型、TS strict | 新增 Tauri command 全部类型化，手写 bindings 与 Rust 类型同步（沿用现有 `src/bindings/` 约定）；tsconfig strict、禁止 `any` | PASS |
| V. 开源协作与透明 | 许可证兼容、无遥测 | FR-019 禁止无关数据收集；若移植 doocs/cose 适配实现先核验其许可证与 MIT 兼容（见 research.md） | PASS |

**门禁结论**：全部通过。无章程违反项，无需进入 Complexity Tracking。WebView 自动化的"平台改版脆弱性"是该方案固有成本，已通过"逐平台失败上报 + 适配器解耦"局部化，不构成原则违反。

## Project Structure

### Documentation (this feature)

```text
specs/002-multi-platform-publish/
├── plan.md              # 本文件
├── research.md          # Phase 0 输出
├── data-model.md        # Phase 1 输出
├── quickstart.md        # Phase 1 输出
├── contracts/           # Phase 1 输出（Tauri command 契约）
│   ├── platform.md      # 平台连接与会话
│   └── publish.md       # 同步、批量、重试、历史
└── tasks.md             # Phase 2 输出（/speckit-tasks，不由本命令生成）
```

### Source Code (repository root)

```text
src/                                  # Vue3 前端（在 001 基础上新增）
├── components/
│   └── publish/                      # 本切片新增
│       ├── PlatformPanel.vue         # 平台连接状态/登录/断开（US1）
│       ├── PublishDialog.vue         # 选文章 + 勾选平台 + 触发同步（US2/US3）
│       ├── SyncResultList.vue        # 逐平台进度/结果/重试（US3）
│       └── SyncHistory.vue           # 文章同步历史（US4）
├── stores/
│   └── publish.ts                    # 平台连接状态 + 同步任务状态（Pinia）
├── services/
│   └── render.ts                     # 用 @md/core 把文章渲染为平台 HTML（与预览同源）
└── bindings/
    ├── commands.ts                   # 追加 publish 相关 command 封装
    └── types.ts                      # 追加 PlatformId/PlatformStatus/SyncJob 等类型

src-tauri/src/                        # Rust 后端（在 001 基础上新增）
├── commands/
│   └── publish.rs                    # 契约入口：连接/状态/断开/同步/重试/历史
├── adapters/                         # 落地 001 预留边界（章程原则 II）
│   ├── mod.rs                        # PublishAdapter trait + 注册表 + MockAdapter（测试）
│   ├── weixin.rs                     # 微信公众号适配器
│   ├── zhihu.rs                      # 知乎适配器
│   └── juejin.rs                     # 掘金适配器
├── publish/                          # 发布编排（不含平台细节）
│   ├── mod.rs
│   ├── session.rs                    # 会话加密落盘 + OS keyring 密钥（FR-005）
│   ├── webview.rs                    # 内嵌 WebView 管理 + JS 注入 + 登录态注入/提取
│   ├── sync.rs                       # SyncJob 编排：渲染→平台化→传图→写草稿；批量/隔离/重试
│   └── history.rs                    # sync_record 读写（SQLite 派生缓存）
└── lib.rs                            # invoke_handler 注册新 command

tests/                                # 前端 Vitest（新增 publish store/组件用例）
```

**Structure Decision**: 沿用 001 的 Tauri 单仓库分层（命令层 → 领域/编排层 → 存储层）。本切片**首次实现 `src-tauri/src/adapters/`**——`PublishAdapter` trait 统一契约，公众号/知乎/掘金各一实现且互不依赖；平台无关的编排（会话、WebView、同步、历史）放在 `publish/`，与 adapter 解耦。前端发布 UI 收敛在 `components/publish/` 与 `stores/publish.ts`，文章渲染复用与编辑器预览同源的 `@md/core`，保证 SC-003 的样式一致性。新增平台只需加一个 adapter 文件并在注册表登记，满足 FR-017 / SC-006。

## Complexity Tracking

> 无章程违反项需要论证。本切片刻意保持简单：不引入官方平台 API 集成（多数平台无开放发布 API）、
> 不引入无头浏览器自动化框架（复用 Tauri 自带 WebView）、不把同步历史升级为第二真相来源（仅派生缓存）、
> 默认只写草稿不自动发布（规避风控与误发）。
