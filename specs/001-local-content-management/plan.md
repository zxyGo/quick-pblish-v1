# Implementation Plan: 本地内容基座（文件管理与文章管理）

**Branch**: `001-local-content-management` | **Date**: 2026-06-21 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `specs/001-local-content-management/spec.md`

## Summary

构建多平台发布桌面应用的"内容基座"切片：用户在本地工作目录中创作、组织、检索 Markdown 文章。
技术上采用 Tauri（Rust 后端）+ Vue3/TypeScript（前端）+ tdesign-vue-next（UI），集成 doocs/md 作为
Markdown 编辑与样式预览核心。元数据以 YAML front matter 内嵌于 `.md` 文件作为唯一真相来源，
SQLite 作为可随时重建的派生缓存用于加速文章列表与检索。所有操作离线可用，删除走系统回收站，
插图复制进工作目录 `assets/` 并以相对路径引用，保存前以哈希比对检测外部修改冲突。

## Technical Context

**Language/Version**: 前端 TypeScript 5.x（Vue 3.4+，strict 模式）；后端 Rust 1.75+（Tauri 2.x）

**Primary Dependencies**:
- 前端：Vue 3、tdesign-vue-next、UnoCSS（原子化样式，非必要不手写 CSS）、Pinia、Vue Router、Vite；doocs/md 编辑器核心（集成方式见 research.md）
- 后端：Tauri 2、serde、gray_matter（front matter 解析）、rusqlite（bundled + FTS5）、trash（回收站）、
  notify（文件监听）、tauri-specta + specta（前后端类型绑定生成）

**Storage**: 本地文件系统（Markdown + `assets/` 素材）为真相来源；SQLite 派生缓存（可重建）用于列表/检索

**Testing**: 后端 `cargo test`（领域逻辑、front matter 解析、索引重建、冲突检测、回收站）；
前端 Vitest（服务层与组件）；关键路径以 Tauri command 契约测试覆盖

**Target Platform**: 桌面端 Windows 10+ / macOS 12+ / Linux（Tauri 支持的发行版）

**Project Type**: desktop-app（Tauri 单仓库：Vue 前端 + Rust 后端）

**Performance Goals**:
- 含 1000 篇文章的工作目录，文章列表首屏可见内容 ≤ 2s（SC-003）
- 编辑器输入到预览样式更新可感知延迟 ≤ 1s（SC-006）

**Constraints**:
- 全部核心操作离线可用（SC-005）
- 跨平台路径/文件名/回收站差异必须处理（章程原则 III）
- 前后端数据契约强类型，TS strict、禁止 `any`（章程原则 IV）

**Scale/Scope**: 单用户单机；典型 0–数千篇文章；本切片 4 个用户故事、20+ 条 FR；不含发布功能

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| 原则 | 要求 | 本计划如何满足 | 状态 |
|------|------|----------------|------|
| I. 本地优先与数据自主 | Markdown 纯文本真相来源、可离线、元数据可迁移、凭据加密 | front matter 为真相来源，SQLite 仅派生缓存可重建；全离线；本切片无凭据 | PASS |
| II. 平台适配器解耦 | 发布平台为独立适配器 | 本切片不含发布，但目录结构预留 `adapters/` 边界，核心不耦合任何平台 | PASS |
| III. 跨平台一致性 | Win/macOS/Linux 一致、不硬编码路径、CI 三平台 | 路径处理统一经 Rust `PathBuf`/Tauri path API；回收站用 `trash` 跨平台 crate；CI 矩阵构建 | PASS |
| IV. 端到端类型安全 | 前后端契约强类型、TS strict、command 类型化 | tauri-specta 由 Rust 单一来源生成 TS 绑定；tsconfig strict；契约见 contracts/ | PASS |
| V. 开源协作与透明 | 许可证兼容、无强制遥测 | doocs/md 已核验为 WTFPL v2，与任何主流开源许可证兼容；无遥测 | PASS |

**门禁结论**：全部通过。doocs/md 许可证（WTFPL v2）已核验，对集成与本项目许可证选择无限制（见 research.md）。
无违反原则需进入 Complexity Tracking。

## Project Structure

### Documentation (this feature)

```text
specs/001-local-content-management/
├── plan.md              # 本文件
├── research.md          # Phase 0 输出
├── data-model.md        # Phase 1 输出
├── quickstart.md        # Phase 1 输出
├── contracts/           # Phase 1 输出（Tauri command 契约）
│   ├── workspace.md
│   ├── article.md
│   ├── file-tree.md
│   └── asset.md
└── tasks.md             # Phase 2 输出（/speckit-tasks，不由本命令生成）
```

### Source Code (repository root)

```text
src/                              # Vue3 前端
├── components/
│   ├── editor/                   # doocs/md 编辑器集成封装（编辑 + 样式预览）
│   ├── file-tree/                # 文件树（FR-012~014）
│   ├── article-list/             # 文章列表 + 检索（FR-015~018）
│   └── workspace/                # 工作目录选择/切换（FR-001~003）
├── views/                        # 页面级组合
├── stores/                       # Pinia 状态（workspace / articles / editor）
├── services/                     # 前端服务层：封装对 Tauri command 的调用
├── bindings/                     # tauri-specta 生成的 TS 绑定（自动生成，勿手改）
├── types/                        # 前端专用类型
├── router/
└── main.ts

src-tauri/                        # Rust 后端
├── src/
│   ├── commands/                 # Tauri command 处理器（契约入口）
│   │   ├── workspace.rs
│   │   ├── article.rs
│   │   ├── file_tree.rs
│   │   └── asset.rs
│   ├── domain/                   # 领域模型与规则（Article / Workspace / Asset）
│   ├── storage/                  # 文件读写 + front matter 解析/序列化 + 哈希
│   ├── index/                    # SQLite 派生缓存（建表、重建、查询、FTS）
│   ├── adapters/                 # 预留：未来发布平台适配器边界（本切片空壳/占位）
│   ├── error.rs                  # 统一错误类型
│   ├── lib.rs
│   └── main.rs
├── Cargo.toml
├── build.rs
└── tauri.conf.json

tests/                            # 前端 Vitest 测试
package.json
vite.config.ts
tsconfig.json
```

**Structure Decision**: 采用 Tauri 标准单仓库布局——前端 `src/`（Vue3）与后端 `src-tauri/`（Rust）。
后端按"命令层 → 领域层 → 存储/索引层"分层，命令层即前后端契约入口；`adapters/` 目录在后端预留，
为后续发布切片的"平台适配器解耦"原则留出边界，本切片不实现。前后端类型由 tauri-specta 单一来源生成，
避免契约漂移。

## Complexity Tracking

> 无章程违反项需要论证。本切片刻意保持简单：不引入 ORM（直接用 rusqlite）、不引入云同步、
> 不实现发布逻辑、SQLite 仅作派生缓存而非第二真相来源。
