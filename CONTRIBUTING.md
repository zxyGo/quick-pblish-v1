# 贡献指南

感谢你对 Quick Publish 的关注！

## 开发流程

本项目采用 [Spec Kit](https://github.com/github/spec-kit) 规格驱动开发：
`specify → plan → tasks → implement`。功能规格位于 [`specs/`](./specs/)。

在动手实现前，请先阅读对应功能的 `spec.md` 与 `plan.md`，并遵循
[项目章程](./.specify/memory/constitution.md) 中的核心原则。

## 环境准备

- Node.js LTS + pnpm
- Rust stable 工具链
- [Tauri 平台依赖](https://tauri.app/start/prerequisites/)

```bash
pnpm install
pnpm tauri dev
```

## 提交前自检

请确保以下全部通过（CI 会在三平台上重复校验）：

```bash
pnpm typecheck
pnpm test
pnpm lint
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml
```

## 约定

- **类型安全**：TypeScript 使用 `strict`，禁止 `any`；前后端契约通过 `src/bindings/` 强类型封装。
- **本地优先**：文章内容始终以 Markdown + front matter 为真相来源，SQLite 仅为可重建的派生缓存。
- **跨平台**：避免硬编码路径分隔符与平台特定行为。
- **提交信息**：清晰描述变更意图。

## 新增发布平台（未来）

发布平台以独立适配器实现，遵循统一契约，不得修改编辑器/文件管理/文章管理核心。
适配器开发规范将随发布切片提供。
