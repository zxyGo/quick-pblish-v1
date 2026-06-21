---
description: "Task list for 本地内容基座（文件管理与文章管理）"
---

# Tasks: 本地内容基座（文件管理与文章管理）

**Input**: Design documents from `specs/001-local-content-management/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/

**Tests**: 按章程"Tauri command 跨语言契约 MUST 有自动化测试覆盖关键路径"，对每个 command 契约纳入
针对性契约测试任务（非全量 TDD）。

**Organization**: 任务按用户故事分组，每个故事可独立实现、独立测试、独立交付。

## Format: `[ID] [P?] [Story] Description`

- **[P]**: 可并行（不同文件、无未完成依赖）
- **[Story]**: 所属用户故事（US1~US4）
- 描述中包含确切文件路径

## Path Conventions

Tauri 单仓库：前端 `src/`，后端 `src-tauri/src/`，前端测试 `tests/`，后端测试在 `src-tauri/`（`cargo test`）。

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: 项目初始化与基础脚手架

- [x] T001 核验 doocs/md 开源许可证与本项目拟用许可证的兼容性 ✅ 结论：doocs/md 为 WTFPL v2，与任何主流开源许可证兼容、无衍生作品同协议要求；已记入 `specs/001-local-content-management/research.md`（章程原则 V 门禁已关闭）
- [ ] T002 用 Tauri 2 脚手架初始化单仓库（前端 `src/` + 后端 `src-tauri/`），配置 `src-tauri/tauri.conf.json`
- [ ] T003 [P] 配置前端：Vue 3 + TypeScript strict、tdesign-vue-next、Pinia、Vue Router、Vite（`package.json`、`tsconfig.json`、`vite.config.ts`，tsconfig 启用 strict 且禁止 `any`）
- [ ] T004 [P] 配置后端依赖于 `src-tauri/Cargo.toml`：serde、gray_matter、rusqlite（bundled+FTS5）、trash、notify、specta、tauri-specta
- [ ] T005 [P] 配置代码规范：前端 ESLint + Prettier，后端 rustfmt + clippy
- [ ] T006 [P] 配置 CI 三平台构建矩阵（Windows/macOS/Linux）于 `.github/workflows/`（章程原则 III）
- [ ] T007 [P] 搭建测试框架：前端 Vitest（`tests/`）、后端 `cargo test` 骨架

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: 所有用户故事都依赖的核心基础设施

**⚠️ CRITICAL**: 本阶段完成前，任何用户故事都不能开始

- [ ] T008 定义统一错误类型 `AppError`（NotFound/Permission/Io/Conflict/Invalid）于 `src-tauri/src/error.rs`，并派生 serde + specta
- [ ] T009 [P] 定义领域模型 Workspace/Article/Asset/FileNode（serde + specta 派生）于 `src-tauri/src/domain/`，对齐 data-model.md
- [ ] T010 实现存储层：front matter 解析/序列化（gray_matter）+ 正文内容哈希于 `src-tauri/src/storage/`，处理缺失/损坏 front matter 的降级（FR-018）
- [ ] T011 实现 SQLite 派生缓存模块于 `src-tauri/src/index/`：建表 `articles` 与 FTS5 `articles_fts`、打开/连接、增量同步、`rebuild_index`、`get_index_status`（FR-008a，存于 OS 应用数据目录）
- [ ] T012 实现工作目录配置持久化（current + recent）于 `src-tauri/src/storage/`（OS 应用配置目录）
- [ ] T013 接入 tauri-specta：在 `src-tauri/src/lib.rs` 注册 command 收集器，构建时生成 TS 绑定到 `src/bindings/`（章程原则 IV，单一真相来源）
- [ ] T014 [P] 前端服务层与 Pinia store 骨架（workspace/articles/editor）于 `src/services/` 与 `src/stores/`，统一封装对 `src/bindings/` 的调用
- [ ] T015 [P] 应用外壳布局（tdesign：侧栏 + 主区）于 `src/views/` 与 `src/router/`

**Checkpoint**: 基础就绪——用户故事可开始

---

## Phase 3: User Story 1 - 创作并保存一篇文章 (Priority: P1) 🎯 MVP

**Goal**: 用户能新建文章、在 doocs/md 编辑器中编辑并实时预览、保存为本地 `.md`，重开可恢复。

**Independent Test**: 选定工作目录后，仅通过"新建→编辑→保存→重开恢复"即可验证（SC-002）。

### Tests for User Story 1 ⚠️（章程契约测试）

- [ ] T016 [P] [US1] 契约测试：`create_article`/`read_article`/`save_article` 往返一致于 `src-tauri/src/commands/article.rs` 的 `#[cfg(test)]`（contracts/article.md）
- [ ] T017 [P] [US1] 单元测试：front matter 解析/序列化 + 正文哈希 + 保存时冲突检测于 `src-tauri/src/storage/`

### Implementation for User Story 1

- [ ] T018 [US1] 实现 `create_article` command 于 `src-tauri/src/commands/article.rs`（写初始 front matter，文件名冲突返回 Conflict，FR-004/FR-020）
- [ ] T019 [US1] 实现 `read_article` command（解析 front matter + 正文，返回 baseHash，FR-005）
- [ ] T020 [US1] 实现 `save_article` command（乐观哈希校验，冲突按 abort/overwrite/saveAs 处理，FR-005/FR-019）并在保存后刷新 SQLite 缓存
- [ ] T021 [US1] 实现 `import_asset` command 于 `src-tauri/src/commands/asset.rs`（复制图片进工作目录 `assets/`，返回相对路径，FR-014a）
- [ ] T022 [P] [US1] 封装 doocs/md 编辑器组件 `EditorPanel.vue` 于 `src/components/editor/`（props 传入正文、事件回传变更、复用其样式预览，文件读写一律走后端）
- [ ] T023 [US1] 编辑器视图：新建/打开/编辑/保存流程 + Dirty 状态 + 未保存提示（FR-007）于 `src/views/` 与 `src/stores/editor`
- [ ] T024 [US1] 外部修改冲突弹窗（覆盖/放弃并重载/另存为）于 `src/components/editor/`（FR-019）
- [ ] T025 [US1] 编辑器插入本地图片 → 调用 `import_asset` 并将相对路径写入 Markdown（FR-014a）

**Checkpoint**: US1 可独立运行与测试——本地 Markdown 写作工具 MVP 成立

---

## Phase 4: User Story 2 - 选择与管理本地工作目录 (Priority: P1)

**Goal**: 用户选择/切换工作目录，应用记住并下次自动加载；目录不可访问时引导重选。

**Independent Test**: 选目录→确认显示→切换→内容随之变化；外部移走目录后重启→引导重选不崩溃（FR-003）。

### Tests for User Story 2 ⚠️（章程契约测试）

- [ ] T026 [P] [US2] 契约测试：`select_workspace`/`get_current_workspace`/`switch_workspace`/`list_recent_workspaces` 于 `src-tauri/src/commands/workspace.rs` 的 `#[cfg(test)]`（contracts/workspace.md）

### Implementation for User Story 2

- [ ] T027 [US2] 实现工作目录 commands（select/get/switch/recent）于 `src-tauri/src/commands/workspace.rs`（不可访问返回 NotFound/Permission，FR-001~003）
- [ ] T028 [US2] workspace store + 启动时自动加载上次目录于 `src/stores/workspace`（FR-001）
- [ ] T029 [US2] 工作目录选择/切换 UI（tdesign）+ 不可访问时的重选流程于 `src/components/workspace/`（FR-002/FR-003）
- [ ] T030 [US2] 切换工作目录时触发 SQLite 缓存对新目录的（重）同步（FR-002，复用 T011）

**Checkpoint**: US1 与 US2 均可独立工作

---

## Phase 5: User Story 3 - 浏览与组织文件树 (Priority: P2)

**Goal**: 用户在侧栏看到与磁盘一致的文件树，可新建文件夹/重命名/移动/删除（入回收站）。

**Independent Test**: 新建文件夹→移入文章→重命名→删除（入系统回收站），核对磁盘真实结构变化（FR-013）。

### Tests for User Story 3 ⚠️（章程契约测试）

- [ ] T031 [P] [US3] 契约测试：`get_file_tree`/`create_folder`/`rename_path`/`move_path`/`delete_path` 于 `src-tauri/src/commands/file_tree.rs` 的 `#[cfg(test)]`（contracts/file-tree.md，含同名冲突 Conflict）

### Implementation for User Story 3

- [ ] T032 [US3] 实现 `get_file_tree` command（结构与磁盘一致，含素材文件，FR-012/FR-014）于 `src-tauri/src/commands/file_tree.rs`
- [ ] T033 [US3] 实现 `create_folder`/`rename_path`/`move_path`/`delete_path`（delete 走 trash 回收站，冲突返回 Conflict，FR-013/FR-020）
- [ ] T034 [US3] 实现 notify 文件监听 → 广播 `workspace_changed` 事件于 `src-tauri/src/`（contracts/file-tree.md）
- [ ] T035 [US3] 文件树组件（tdesign Tree）+ 右键菜单 + 删除确认于 `src/components/file-tree/`（FR-013，删除前确认）
- [ ] T036 [US3] 前端订阅 `workspace_changed` 事件刷新文件树/列表

**Checkpoint**: US1、US2、US3 均可独立工作

---

## Phase 6: User Story 4 - 文章列表、元数据与检索 (Priority: P2)

**Goal**: 用户在列表看到文章及元数据，可按标题/标签/正文检索、按时间排序、编辑标题与标签。

**Independent Test**: 准备多篇文章→检索→排序→改标签后再过滤；损坏 front matter 仍能加载（FR-018）。

### Tests for User Story 4 ⚠️（章程契约测试）

- [ ] T037 [P] [US4] 契约测试：`list_articles`（检索/排序）、`update_metadata`、损坏元数据降级于 `src-tauri/src/commands/article.rs` 的 `#[cfg(test)]`（contracts/article.md）

### Implementation for User Story 4

- [ ] T038 [US4] 实现 `list_articles` command（FTS5 检索 + 排序，从派生缓存读取，降级损坏元数据，FR-015/FR-017/FR-018）
- [ ] T039 [US4] 实现 `update_metadata` command（写 front matter title/tags 并刷新缓存，FR-016）
- [ ] T040 [US4] 文章列表组件（tdesign List/Table）+ 检索框 + 排序 + 标签过滤于 `src/components/article-list/`（FR-015/FR-017）
- [ ] T041 [US4] 标签与标题编辑 UI → 调用 `update_metadata`（FR-016）

**Checkpoint**: 全部用户故事均可独立工作

---

## Phase 7: Polish & Cross-Cutting Concerns

**Purpose**: 跨故事的完善、开源就绪与验收

- [ ] T042 [P] 开源就绪：补充 `README.md`、`CONTRIBUTING.md`（`LICENSE` 已就绪：MIT，Copyright © 2026 zxyGo；README 中保留对 doocs/md WTFPL 的致谢）（章程原则 V）
- [ ] T043 [P] 空态/加载态/错误态的统一 UI 处理（列表为空、目录无文章、保存失败等）
- [ ] T044 性能抽查：生成 1000 篇文章验证列表首屏 ≤ 2s（SC-003），必要时优化缓存/分页
- [ ] T045 离线验证：断网下跑通新建/编辑/保存/组织/检索（SC-005）
- [ ] T046 派生缓存可重建验证：删除 SQLite 缓存后重启自动重建无数据丢失（FR-008a）
- [ ] T047 执行 `specs/001-local-content-management/quickstart.md` 全部验证场景
- [ ] T048 [P] 跨平台抽查：在 Windows/macOS/Linux 各验证路径/回收站/文件名差异（章程原则 III）

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: 无依赖，可立即开始（T001 许可证核验是 doocs/md 集成相关任务 T022 的前置）
- **Foundational (Phase 2)**: 依赖 Setup 完成 —— 阻塞所有用户故事
- **User Stories (Phase 3–6)**: 均依赖 Foundational 完成
  - US1、US2 同为 P1，建议先 US1（MVP）再 US2；US2 的工作目录能力可独立测试
  - US3、US4（P2）依赖 Foundational，可在 P1 之后并行
- **Polish (Phase 7)**: 依赖目标用户故事完成

### User Story Dependencies

- **US1 (P1)**: Foundational 后即可开始；测试时需先选定一个工作目录（可用 US2 能力或测试夹具）
- **US2 (P1)**: Foundational 后即可开始，独立可测
- **US3 (P2)**: Foundational 后即可开始，独立可测
- **US4 (P2)**: Foundational 后即可开始；依赖 T011 缓存模块（已在 Foundational）

### Within Each User Story

- 契约测试先写并失败 → 再实现 command
- 后端 command 先于前端 UI 集成
- 模型/存储先于服务/命令

### Parallel Opportunities

- Setup 中 T003~T007 标 [P] 可并行
- Foundational 中 T009、T014、T015 标 [P] 可并行（T008/T010/T011/T012/T013 有依赖关系，顺序进行）
- Foundational 完成后，US1–US4 可由不同成员并行推进
- 各故事内标 [P] 的契约测试可并行

---

## Parallel Example: User Story 1

```bash
# 契约/单元测试并行编写：
Task: "契约测试 create/read/save 往返于 src-tauri/src/commands/article.rs"
Task: "单元测试 front matter 解析 + 哈希于 src-tauri/src/storage/"

# 前端编辑器封装与后端 command 可并行：
Task: "封装 EditorPanel.vue 于 src/components/editor/"
Task: "实现 create_article/read_article/save_article 于 src-tauri/src/commands/article.rs"
```

---

## Implementation Strategy

### MVP First (User Story 1 + 工作目录最小能力)

1. 完成 Phase 1 Setup
2. 完成 Phase 2 Foundational（阻塞所有故事）
3. 完成 Phase 3 US1（必要时借用 US2 的最小选目录能力）
4. **STOP & VALIDATE**: 独立验证 US1（新建→编辑→保存→重开恢复）
5. 可演示

### Incremental Delivery

1. Setup + Foundational → 基础就绪
2. US1 → 独立验证 → 演示（MVP）
3. US2 → 工作目录管理完善
4. US3 → 文件树组织
5. US4 → 列表与检索
6. 每个故事增量交付，不破坏既有故事

---

## Notes

- [P] = 不同文件、无依赖
- [Story] 标签用于追溯到具体用户故事
- doocs/md 许可证核验（T001）必须先于其集成（T022）
- SQLite 始终是派生缓存，任何任务都不得让它成为元数据真相来源（FR-008a）
- 提交粒度：每个任务或逻辑组完成后提交
- 在每个 Checkpoint 停下独立验证故事
