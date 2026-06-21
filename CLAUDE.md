<!-- SPECKIT START -->
For additional context about technologies to be used, project structure,
shell commands, and other important information, read the current plan:
`specs/001-local-content-management/plan.md`

Active feature: 001-local-content-management（本地内容基座：文件管理与文章管理）
Stack: Tauri 2 + Rust（后端）/ Vue 3 + TypeScript strict + tdesign-vue-next（前端）/ doocs/md（编辑器）。
真相来源：本地 Markdown（YAML front matter）；SQLite 仅为可重建的派生缓存。
契约：见 specs/001-local-content-management/contracts/，前后端类型由 tauri-specta 生成。
<!-- SPECKIT END -->
