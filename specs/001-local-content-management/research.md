# Research: 本地内容基座

**Feature**: 001-local-content-management | **Date**: 2026-06-21

本文件汇总 Phase 0 的技术决策，解决 plan.md Technical Context 中的不确定项。
格式：Decision / Rationale / Alternatives considered。

---

## 1. doocs/md 编辑器集成方式

- **Decision**: 以**封装组件**方式集成 doocs/md 的编辑 + 渲染核心：将其作为前端依赖（vendored 子目录或
  npm/子模块形式）引入，外层用一个 `EditorPanel.vue` 封装，通过 props 传入文章正文、通过事件回传变更，
  样式预览复用 doocs/md 的渲染管线。文件读写一律由本应用的 Tauri 后端负责，doocs/md 仅负责"编辑与预览"，
  不直接触碰文件系统。
- **Rationale**: doocs/md 是成熟的 Vue3 Markdown 编辑器，复用其编辑体验与样式渲染能省下大量工作；
  把"读写文件"职责收归后端，符合本地优先与端到端类型安全原则，也避免前端绕过契约直接操作磁盘。
- **Alternatives considered**:
  - iframe 嵌入整个 doocs/md SPA：隔离性好但通信成本高、样式/状态同步麻烦，且难以贯彻统一 UI。
  - 从零自建编辑器（CodeMirror + markdown-it）：可控但重复造轮子，放弃了用户明确要求的 doocs/md。
- **License（已核验，门禁关闭）**: doocs/md 采用 **WTFPL v2**（Copyright © 2025 Doocs <admin@doocs.org>）。
  WTFPL 是最宽松的许可证之一，对使用/修改/再分发无实质限制，**不要求衍生作品同协议开源**，与本项目选用
  任何主流开源许可证（MIT / Apache-2.0 / GPL 等）均兼容（章程原则 V 满足）。
  建议：尽管 WTFPL 不强制署名，集成时仍保留对 doocs/md 的致谢。
  **本项目许可证已定：MIT**（Copyright © 2026 zxyGo，`LICENSE` 已创建于仓库根）。

## 2. 前后端类型契约同步

- **Decision**: 使用 **tauri-specta + specta**，由 Rust 端 command 签名与类型作为**单一真相来源**，
  构建时生成 TypeScript 绑定到 `src/bindings/`，前端只调用生成的强类型函数。
- **Rationale**: 直接满足章程原则 IV（前后端类型同步、command 强类型、单一来源生成），消除手写 `invoke`
  字符串与手抄类型导致的契约漂移。
- **Alternatives considered**:
  - ts-rs：仅生成类型不生成调用绑定，仍需手写 invoke 包装。
  - 手写类型 + 手写 invoke：维护成本高、易漂移，违背原则 IV 的"单一来源"。

## 3. 元数据承载与解析（front matter）

- **Decision**: 文章元数据以 **YAML front matter** 内嵌于 `.md` 头部，Rust 端用 **gray_matter** crate
  解析/读取，序列化时保证 front matter 与正文稳定拼接。front matter 字段见 data-model.md。
- **Rationale**: 与 Clarifications 决策 Q1/Q2 一致，front matter 为唯一真相来源、随文件迁移；
  gray_matter 是 Rust 生态成熟的 front matter 解析库。
- **Alternatives considered**:
  - 手写 `---` 分割 + serde_yaml：可行但需自行处理各种边界（无 front matter、CRLF、BOM），不如成熟库稳健。
  - sidecar / 集中数据库为真相来源：已在 Clarifications 中否决（违反 FR-008）。

## 4. SQLite 派生缓存

- **Decision**: 使用 **rusqlite（bundled 特性，内置 SQLite）** 在应用数据目录维护派生缓存，
  存文章元数据与正文用于列表/检索；全文检索用 **SQLite FTS5**。缓存对每篇文件记录
  `path + size + mtime + content_hash`，启动时与磁盘比对做增量同步；提供"完全重建"命令。
  缓存损坏或缺失时直接重建，绝不阻断功能。
- **Rationale**: 满足 SC-003（1000 篇 ≤ 2s）与检索成功率（SC-004），同时严格保持"派生、可重建、非真相来源"
  （FR-008a）。bundled 特性避免对系统 SQLite 的依赖，利于跨平台一致性（原则 III）。
- **Alternatives considered**:
  - sqlx：异步/编译期校验强，但对单机嵌入式缓存偏重，且需要连接串配置，rusqlite 更轻。
  - 纯内存索引（每次启动全量扫描）：实现简单但冷启动慢、检索弱，难达 SC-003。
  - 缓存目录放工作目录内：会污染用户内容目录，改放 OS 应用数据目录（Tauri path API）。

## 5. 跨平台回收站（删除策略）

- **Decision**: 删除文件/文件夹使用 **trash** crate 移入操作系统回收站（Q3 决策）；删除前由前端弹确认。
- **Rationale**: 跨平台统一封装 Windows / macOS / Linux 的回收站语义，满足原则 III 与 FR-006 的可恢复性。
- **Alternatives considered**:
  - 永久删除：不可恢复，已在 Q3 否决。
  - 应用内自建回收站目录：增加复杂度且与系统回收站语义割裂，用户预期不一致。

## 6. 外部修改冲突检测

- **Decision**: 保存采用 **乐观并发 + 哈希校验**：前端打开文章时后端返回 `base_hash`（正文内容哈希）；
  保存时携带 `base_hash`，后端在写入前重新读取磁盘文件计算当前哈希，若与 `base_hash` 不一致则返回
  `Conflict` 错误，前端据此弹窗让用户选择 覆盖 / 放弃本地并重载 / 另存为（Q5、FR-019）。
  另用 **notify** crate 监听工作目录变更以驱动文件树/列表的实时刷新（非编辑中文件可提示或自动刷新）。
- **Rationale**: 哈希乐观校验实现简单且可靠地"禁止静默覆盖"；notify 提升一致性体验但不作为冲突判定依据
  （避免竞态），二者职责分离。
- **Alternatives considered**:
  - 仅靠 mtime 比对：mtime 精度/时区/被外部工具重写为旧值等问题，可靠性不如内容哈希。
  - 文件锁：跨平台语义不一致、易留下死锁残留，不适合本地单用户场景。

## 7. 图片/素材导入

- **Decision**: 编辑器插入本地图片时，由后端 `import_asset` command 将图片复制进工作目录的 `assets/`
  目录（按需子目录组织），返回相对路径，前端把相对路径写入 Markdown（Q4、FR-014a）。
- **Rationale**: 保证"文章 + 素材"可整体迁移、离线可用，符合本地优先原则。
- **Alternatives considered**: 绝对路径引用（移动文章断链）、base64 内嵌（文件膨胀、预览慢）——均已在 Q4 否决。

## 8. 状态管理与 UI

- **Decision**: 前端用 **Pinia** 管理 workspace / articles / editor 三块状态；UI 组件优先复用
  **tdesign-vue-next** 的 Tree、List、Dialog、Input 等基础组件与设计令牌（章程技术栈约束）。
- **Rationale**: Pinia 是 Vue3 官方推荐状态库；tdesign 复用满足统一设计语言要求。
- **Alternatives considered**: Vuex（已被 Pinia 取代）；自建组件（违背"优先复用 tdesign"约束）。

## 9. 工作目录持久化与"最近使用"

- **Decision**: 当前工作目录与最近使用列表存于 Tauri 应用配置（OS 配置目录），启动时自动加载上次目录；
  目录不可访问时回退到引导用户重新选择（FR-001~003）。
- **Rationale**: 配置属应用级偏好，不应放进用户内容目录；满足 FR-001 的"记住选择、下次自动加载"。
- **Alternatives considered**: 放工作目录内（污染内容、切换目录即丢配置）。

---

## 门禁项状态（进入实现前）

1. ~~**doocs/md 许可证核验**（章程原则 V）~~ —— ✅ 已关闭：doocs/md 为 WTFPL v2，与任何主流开源许可证兼容，
   无衍生作品同协议开源要求（见上节 1）。

所有 Technical Context 不确定项与门禁项均已解决，无遗留 NEEDS CLARIFICATION。
