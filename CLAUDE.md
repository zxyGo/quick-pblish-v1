<!-- SPECKIT START -->
For additional context about technologies to be used, project structure,
shell commands, and other important information, read the current plan:
`specs/002-multi-platform-publish/plan.md`

Active feature: 002-multi-platform-publish（多平台发布：浏览器同步式一键存草稿）
Stack: Tauri 2 + Rust（后端）/ Vue 3 + TypeScript strict + tdesign-vue-next（前端）/ doocs/md 渲染核心。
机制：内嵌 WebView 复用各平台登录态 + 注入 JS 写草稿；平台为 `src-tauri/src/adapters/` 下独立适配器（公众号/知乎/掘金）。
会话凭据 AES-GCM 加密落盘、密钥存 OS keyring；同步历史为 SQLite 派生缓存；仅写草稿不自动发布。
契约：见 specs/002-multi-platform-publish/contracts/。前序基座：001-local-content-management。
<!-- SPECKIT END -->
