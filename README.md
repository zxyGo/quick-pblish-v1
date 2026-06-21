# Quick Publish

> 多平台内容发布桌面应用 —— 本地优先的 Markdown 写作与文章管理，面向多平台一键发布（开发中）。

[![CI](https://github.com/zxyGo/quick-publish/actions/workflows/ci.yml/badge.svg)](./.github/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](./LICENSE)

## 这是什么

Quick Publish 是一个跨平台（Windows / macOS / Linux）桌面应用，帮助内容创作者在本地完成：

- **本地优先的文章管理**：文章以 Markdown 纯文本存储，元数据内嵌于 front matter，随文件可迁移、可备份、可离线编辑。
- **文件与素材组织**：内置文件树，管理工作目录里的文章与图片素材。
- **Markdown 创作与样式预览**：集成 [doocs/md](https://github.com/doocs/md) 的编辑与样式渲染能力。
- **多平台发布**（规划中）：以可插拔的平台适配器架构，逐步支持发布到多个内容平台。

当前已落地「本地内容基座」切片（文件管理 + 文章管理 + 编辑器集成）。规格与计划见
[`specs/001-local-content-management/`](./specs/001-local-content-management/)。

## 技术栈

| 层 | 技术 |
|----|------|
| 应用框架 | [Tauri 2](https://tauri.app/)（Rust 后端 + WebView 前端）|
| 前端 | Vue 3 + TypeScript（strict）+ Vite |
| UI | [tdesign-vue-next](https://tdesign.tencent.com/vue-next/) |
| 编辑器 | [doocs/md](https://github.com/doocs/md) |
| 存储 | 本地 Markdown（front matter 为真相来源）+ SQLite 派生缓存（可重建）|

## 核心设计原则

详见 [项目章程](./.specify/memory/constitution.md)：

1. **本地优先与数据自主** —— Markdown 纯文本为唯一真相来源，离线可用。
2. **平台适配器解耦** —— 每个发布平台是独立适配器，新增平台不改核心。
3. **跨平台一致性** —— Win/macOS/Linux 一致，CI 三平台构建。
4. **端到端类型安全** —— 前后端契约强类型。
5. **开源协作与透明** —— 无强制遥测。

## 开发

前置：Node.js LTS、pnpm、Rust 工具链，以及 [Tauri 平台依赖](https://tauri.app/start/prerequisites/)。

```bash
pnpm install         # 安装前端依赖
pnpm tauri dev       # 启动桌面开发窗口

pnpm typecheck       # 前端类型检查
pnpm test            # 前端单元测试
cargo test --manifest-path src-tauri/Cargo.toml   # 后端测试
```

## 开源协作

欢迎贡献，详见 [CONTRIBUTING.md](./CONTRIBUTING.md)。

## 许可证与致谢

本项目基于 [MIT License](./LICENSE) 开源。

集成的 [doocs/md](https://github.com/doocs/md) 采用 WTFPL 协议，特此致谢。
