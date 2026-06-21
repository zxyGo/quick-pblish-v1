# Vendored: doocs/md 渲染核心

本目录以源码形式 vendored 了 [doocs/md](https://github.com/doocs/md) 的**渲染核心**，
为本应用的 Markdown 样式预览（`src/components/editor/EditorPanel.vue`，任务 T022）提供动力。

## 来源

- 仓库：https://github.com/doocs/md
- 版本：`v2.1.0`
- Commit：`ac03473b1c15aa81c8cc3a7d1613a8ea75f0000b`（2026-06-20）
- 许可证：**WTFPL v2**（见 `LICENSE`），与本项目 MIT 许可证兼容。
  详见 `specs/001-local-content-management/research.md` 第 1 节的许可证核验结论。

## 包含内容

- `packages/core/` —— `@md/core`：marked 渲染器、主题系统、Markdown 扩展（alert/footnotes/
  katex/mermaid/plantuml/infographic 等）、工具函数。
- `packages/shared/` —— `@md/shared`：渲染核心依赖的类型、配置（含主题 CSS、默认样式）、
  阅读时间等工具。

通过根目录 `pnpm-workspace.yaml` 将二者作为工作区包链接，主应用以 `@md/core` / `@md/shared`
import 使用。

## 相对上游的改动（仅集成所需，未改渲染逻辑）

1. 删除各包 `package.json` 的 `devDependencies` 与 `scripts`（本项目不运行其独立测试/类型检查）。
2. 各包 `tsconfig.json` 原 `extends: "@md/config/..."` 改为内联自包含配置（未 vendor `@md/config`）。
3. 删除随包的 `*.test.ts`。

## 升级方式

重新从上游对应 commit 拉取 `packages/core`、`packages/shared`，再套用上述三处改动即可。
