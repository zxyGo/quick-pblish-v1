<!--
Sync Impact Report
==================
Version change: (template/unversioned) → 1.0.0
Bump rationale: Initial ratification of the project constitution (MAJOR baseline).

Modified principles:
  - [PRINCIPLE_1_NAME] → I. 本地优先与数据自主 (Local-First & Data Ownership)
  - [PRINCIPLE_2_NAME] → II. 平台适配器解耦 (Platform Adapter Decoupling)
  - [PRINCIPLE_3_NAME] → III. 跨平台一致性 (Cross-Platform Consistency)
  - [PRINCIPLE_4_NAME] → IV. 端到端类型安全 (End-to-End Type Safety)
  - [PRINCIPLE_5_NAME] → V. 开源协作与透明 (Open-Source Collaboration & Transparency)

Added sections:
  - 技术栈约束 (Technology Stack Constraints)
  - 开发工作流与质量门禁 (Development Workflow & Quality Gates)

Removed sections: none

Templates requiring updates:
  - .specify/templates/plan-template.md ✅ reviewed (Constitution Check 为通用占位，无冲突)
  - .specify/templates/spec-template.md ✅ reviewed (无强制章节冲突)
  - .specify/templates/tasks-template.md ✅ reviewed (任务分类与原则兼容)

Deferred TODOs: none
-->

# 多平台发布桌面应用 Constitution

## Core Principles

### I. 本地优先与数据自主 (Local-First & Data Ownership)

所有用户内容（草稿、生成的文章、素材、平台凭据）MUST 以可读、可移植的格式优先存储在本地文件系统。

- 文章正文 MUST 以 Markdown 纯文本存储；元数据 MUST 与正文一同可导出，不得锁定在不透明的二进制库中。
- 应用 MUST 在完全离线状态下完成编辑、管理、组织等核心操作；仅"发布"动作允许依赖网络。
- 凭据等敏感数据 MUST 加密存储于操作系统提供的安全设施（如 keychain/credential store），禁止明文落盘。

**Rationale**: 内容创作者的资产不应被工具绑架。本地优先保证数据可迁移、可备份、可审计，也是开源工具赢得信任的前提。

### II. 平台适配器解耦 (Platform Adapter Decoupling)

每一个发布目标平台 MUST 实现为独立、自包含的适配器，遵循统一的发布契约接口。

- 新增一个平台 MUST 不需要修改编辑器、文件管理或文章管理的核心代码。
- 适配器之间 MUST NOT 相互依赖；每个适配器 MUST 可被独立测试（鉴权、内容转换、发布、错误处理）。
- 平台特有的内容转换（如不同平台的 Markdown 方言、图片上传策略）MUST 封装在各自适配器内部。

**Rationale**: "多平台"是本项目核心价值。适配器模式让平台扩展成本最低，也便于社区为新平台贡献插件。

### III. 跨平台一致性 (Cross-Platform Consistency)

应用 MUST 在 Windows、macOS、Linux 上提供一致的功能与体验。

- 任何文件系统、路径、快捷键、系统集成相关代码 MUST 处理三平台差异，禁止硬编码平台特定路径或分隔符。
- 功能 MUST NOT 仅在单一平台可用；若某能力受平台限制，MUST 显式降级并提示，而非静默失败。
- 发布产物 MUST 通过 CI 在三平台构建验证后才可发版。

**Rationale**: Tauri 的核心优势是跨平台。一致性是承诺，必须在 CI 层强制保障，而非依赖人工记忆。

### IV. 端到端类型安全 (End-to-End Type Safety)

前端（Vue3/TypeScript）与后端（Rust/Tauri）之间的所有数据契约 MUST 强类型化。

- TypeScript MUST 启用 `strict` 模式；禁止使用 `any` 规避类型系统（确有需要时 MUST 显式注释理由）。
- Tauri command 的入参与返回 MUST 有明确的类型定义，前后端类型 MUST 保持同步（优先由单一来源生成）。
- 公开的核心模块接口 MUST 有类型签名与文档，作为契约的一部分。

**Rationale**: 跨语言边界是缺陷高发区。强类型把运行时错误提前到编译期，是小团队/社区维护质量的低成本保障。

### V. 开源协作与透明 (Open-Source Collaboration & Transparency)

项目以开源方式运作，所有决策与变更 MUST 可追溯、可复现。

- 仓库 MUST 包含开源许可证、README、贡献指南；提交信息 MUST 清晰描述意图。
- 任何依赖引入 MUST 与开源许可证兼容，并在文档中可见。
- 面向用户的变更 MUST 记录在 changelog；破坏性变更 MUST 显著标注。
- MUST NOT 收集用户遥测数据，除非明确告知并默认关闭、可由用户主动开启。

**Rationale**: 开源项目的生命力来自信任与可参与性。透明的流程与隐私尊重是吸引贡献者和用户的根基。

## 技术栈约束 (Technology Stack Constraints)

以下技术选型为项目基线，偏离 MUST 在 plan 阶段记录理由并通过评审：

- **应用框架**: Tauri（Rust 后端 + WebView 前端），目标平台 Windows / macOS / Linux。
- **前端框架**: Vue 3（Composition API）+ TypeScript（strict）。
- **UI 组件库**: tdesign-vue-next，作为统一设计语言来源；自定义组件 MUST 优先复用其基础组件与设计令牌。
- **Markdown 编辑器**: 集成 [doocs/md](https://github.com/doocs/md) 作为编辑与多平台样式渲染核心；集成方式 MUST 尊重其许可证。
- **存储**: 本地文件系统（Markdown + 元数据）；如需索引/查询，SHOULD 使用嵌入式方案（如 SQLite），且 MUST NOT 成为内容的唯一真相来源。

## 开发工作流与质量门禁 (Development Workflow & Quality Gates)

- **规格驱动**: 功能开发 MUST 遵循 Spec Kit 流程：specify → plan → tasks → implement。
- **质量门禁**: 合入主分支前 MUST 通过类型检查（tsc）、前端构建、Rust 编译/clippy；三平台 CI 构建 MUST 全绿。
- **测试要求**: 平台适配器与跨语言契约（Tauri command）MUST 有自动化测试覆盖关键路径。
- **评审**: 所有变更 MUST 经过代码评审，评审者 MUST 核验与本章程的符合性。

## Governance

本章程是项目的最高约束，优先级高于其他实践与约定；冲突时以本章程为准。

- **修订流程**: 章程修订 MUST 通过 PR 提出，说明动机与影响，经评审合入；合入后 MUST 更新版本号与修订日期，并同步相关模板。
- **版本策略**: 遵循语义化版本——MAJOR 用于原则的移除或不兼容重定义，MINOR 用于新增原则或实质性扩充，PATCH 用于澄清与措辞修正。
- **合规审查**: 每次 PR 评审 MUST 核验对本章程的符合性；引入额外复杂度 MUST 在 plan 中说明理由。运行时开发指引以各 agent 上下文文件为准。

**Version**: 1.0.0 | **Ratified**: 2026-06-21 | **Last Amended**: 2026-06-21
