# Phase 0 Research: 多平台发布（浏览器同步式一键发布）

本文件解决 plan.md Technical Context 中的未知项，给出关键技术决策、理由与备选。

## R1. 桌面应用如何复用平台登录态（核心机制）

**Decision**: 在 Tauri 内为每个平台开**独立内嵌 WebView**（`WebviewWindowBuilder`，可隐藏/按需显示），
用户在其中以平台原生方式登录；会话 Cookie 由系统 WebView 持有。发布时通过 `webview.eval(js)` /
初始化脚本向该 WebView **注入 JS**，复用其登录态调用平台自身的内部接口（fetch/XHR 带 Cookie）或操作 DOM
写入草稿。这是 doocs/cose 浏览器扩展 content-script 机制在桌面端的等价实现。

**Rationale**:
- 桌面应用拿不到系统浏览器（Chrome/Edge）的 Cookie，扩展形态无法照搬；但 Tauri 自带系统 WebView，
  能在应用内承载一个"受控浏览器上下文"，天然持有自己的会话。
- 复用平台内部接口/DOM，避免依赖各平台开放 API（公众号有 API 但需 AppID/AppSecret 且能力受限；知乎、
  小红书等无开放发布 API）。这是覆盖多平台的唯一通用路径。
- 注入 JS 在平台页面上下文执行，等同用户手动操作，最大程度复用平台前端已有的鉴权、加签、上传逻辑。

**Alternatives considered**:
- 浏览器扩展（cose 原方案）：与本项目"桌面应用"形态不符，且要求用户装扩展，放弃。
- Rust 侧 HTTP 客户端 + 从 WebView 导出 Cookie 重放内部接口：需复刻各平台加签/防盗链/CSRF，极脆弱，放弃为默认；仅在个别平台 DOM 不可行时作为 adapter 内部备选。
- 无头浏览器（Playwright 等）外部进程：引入重依赖、分发与一致性成本高，与 Tauri 自带 WebView 重复，放弃。

## R2. 跨平台 WebView 差异与一致性（章程原则 III）

**Decision**: 在 `publish/webview.rs` 建立**统一抽象层**封装三种系统 WebView 的差异：WebView2（Windows）、
WKWebView（macOS）、WebKitGTK（Linux）。对外只暴露 `open_login(platform)`、`eval(platform, js) -> 结果`、
`navigate`、`probe_login` 等稳定接口。Linux WebKitGTK 在某些注入/Cookie 持久化行为上较弱，受限处**显式降级提示**
（"该平台在当前系统暂不支持/需手动操作"），禁止静默失败。

**Rationale**: Tauri 的 `eval` 与多 WebView 在三平台均可用，但 JS↔Rust 取值方式、Cookie 持久化细节有差异。
抽象层把差异内聚一处，adapter 只面向稳定接口编写，符合原则 III 与 II。

**Alternatives considered**: 各 adapter 直接调 Tauri WebView API —— 差异散落、难维护，放弃。

## R3. 登录态的加密落盘与密钥保管（FR-005 / Clarification Q2）

**Decision**: 采用"**加密 blob + OS 安全设施密钥**"方案：
- 登录成功后，从该平台 WebView 提取持久化所需的会话数据（Cookie 等）序列化为 blob。
- 用 `aes-gcm`（256 位）加密 blob，**密钥经 `keyring` crate 存入 OS 安全设施**（Windows Credential Manager /
  macOS Keychain / Linux Secret Service）。密文 blob 落盘于 app data 目录。
- 下次启动：从 keyring 取密钥解密 blob，**回灌（rehydrate）**到平台 WebView 恢复登录态。
- WebView 自身的数据目录视为**可重建缓存**，断开连接（FR-004）时连同 keyring 中密钥一并清除。

**Rationale**: 系统 WebView 的数据目录在使用中无法被我们整体加密；以"提取-加密-回灌"的方式，使**唯一持久真相**
是受 OS 密钥保护的密文 blob，满足"会话凭据禁止明文落盘 + 密钥存 OS 安全设施"（Q2 决策与章程原则 I），且实现可行。

**Alternatives considered**:
- 直接加密 WebView 用户数据目录：运行中文件被占用、跨平台格式不一，不可行。
- 仅依赖 OS 用户隔离、明文存数据目录：违反 FR-005 措辞，放弃。
- Tauri Stronghold 插件：能力足够但偏重（IOTA Stronghold 快照），单密钥场景用 `keyring` 更轻，作为备选保留。

## R4. 登录状态检测（FR-003 / FR-006）

**Decision**: 每个 adapter 提供 `probe_login_js` —— 在平台 WebView 注入一段探测 JS（请求平台"当前用户/账号信息"
内部端点或读取已登录态特征 DOM），据返回判定 `未连接 / 已连接(账号标识) / 需重新登录`。应用在打开发布 UI 时
与同步前各探测一次。

**Rationale**: 复用登录态最可靠的判定是"以当前会话访问一个需登录的轻量端点"。账号标识同源获取可直接展示（FR-003）。

**Alternatives considered**: 仅看 Cookie 是否存在 —— Cookie 在但已过期会误判为已连接，违反 FR-006，放弃。

## R5. 文章渲染与平台样式保真（FR-009 / SC-003）

**Decision**: 前端用项目已集成的 `@md/core`（doocs/md 渲染核心）把文章 Markdown 渲染为**带内联样式的 HTML**——
与编辑器预览**同源同样式**，保证一致性。该 HTML 作为 `sync_article` 入参传给后端；各 adapter 再做平台化微调
（公众号需内联样式的整段 HTML；知乎/掘金接受其编辑器可识别的 HTML/富文本）。平台特有的转换封装在各 adapter
内部（FR-011）。

**Rationale**: doocs/md 的核心价值就是"为公众号生成内联样式 HTML"，直接复用即可达成 SC-003 的 ≥95% 一致率；
渲染在前端、平台化转换在 adapter，既同源又满足解耦。

**Alternatives considered**: 在 Rust 侧重新实现 Markdown→HTML —— 与编辑器预览不同源、必然样式漂移，放弃。

## R6. 文中插图上传与替换（FR-010 / FR-010a / SC-005）

**Decision**: 写草稿前，对 HTML 中引用本地 `assets/` 的图片逐张处理：后端读取图片字节（base64）交给 adapter 的
`upload_image_js`，在平台 WebView 内复用会话调用平台**自身的上传端点**，得到平台可访问 URL，回填替换
`src`。任一图片上传失败 → 该平台本次同步整体判失败并给出原因，**不生成不完整草稿**（FR-010a），用户修复后可重试。

**Rationale**: 复用平台自有上传端点可绕过防盗链（平台只接受自家 CDN 图片）；"全有或全无"保证草稿不出现坏图、
不残留本地路径（SC-005）。

**Alternatives considered**: 把本地图片转 data-URI 内联 —— 多数平台编辑器会丢弃/不渲染 data-URI，放弃。带占位继续 —— 违反 Q4 决策，列为后续可选增强。

## R7. 草稿写入与"每次新建"（FR-008 / FR-016a）

**Decision**: 各 adapter 的 `save_draft_js` 调用平台**保存草稿**的内部接口（绝不触发"发布/群发"），每次**新建**
一条草稿，不定位或覆盖历史草稿（FR-016a）。返回平台草稿标识/链接，供"前往平台查看草稿"。

**Rationale**: 只到草稿、每次新建，最大限度规避风控与误覆盖用户在平台的手改（Q3/草稿优先决策）。

**Alternatives considered**: 更新已有草稿 —— 需跨平台定位历史草稿，复杂且有误覆盖风险，列为后续增强。

## R8. 批量同步、隔离与重试（FR-013~FR-016）

**Decision**: `publish/sync.rs` 把一次批量同步拆成多个 `SyncJob`（每平台一个），**串行**经"同一受控 WebView 依次
导航到各平台"执行（降低多 WebView 内存占用，满足 SC-001 单平台 ≤60s 即可）。每个 job 独立 try，单平台失败仅标记
该 job（FR-015），不影响其余；失败 job 可单独重试（FR-016）。进度/结果经 Tauri 事件流式推送前端（FR-014）。

**Rationale**: 串行 + 事件推送实现简单、资源可控、隔离清晰；平台风控也更偏好非并发。

**Alternatives considered**: 多平台并发多 WebView —— 内存高、易触发风控、收益有限，放弃（保留为后续优化项）。

## R9. 同步历史存储（FR-018）

**Decision**: 在现有 SQLite 派生缓存中新增表 `sync_record`（文章相对路径、平台、状态、原因、草稿引用、时间戳）。
属派生数据，可清空/重建，**非真相来源**（章程原则 I）。

**Rationale**: 历史是辅助排查信息，丢失不影响内容资产，放派生缓存最合适，复用 001 已有的 rusqlite 设施。

**Alternatives considered**: 写入文章 front matter —— 会污染真相来源且并发写复杂，放弃。

## R10. 类型契约与 bindings（章程原则 IV）

**Decision**: 沿用现有约定——后端定义类型化 Tauri command，前端在手写的 `src/bindings/`（`commands.ts` +
`types.ts`）追加对应封装与类型；错误沿用既有 `AppError` 枚举并按需扩充 `kind`（如 `Auth`/`Network`/`Platform`）。
TS strict、禁止 `any`。

**Rationale**: 与 001 现状一致（bindings 当前为手写而非 specta 生成），保持单一调用层、杜绝裸 `invoke` 字符串。

**Alternatives considered**: 立刻引入 tauri-specta 自动生成 —— 属独立改造，超出本切片范围，记为后续技术债项。

## R11. 许可证合规（章程原则 V）

**Decision**: 若参考/移植 doocs/cose 的任一平台适配实现（选择子/接口路径/JS 片段），MUST 先核验其 LICENSE 与本项目
MIT 兼容并保留出处声明；否则仅参考其公开行为、独立实现。

**Rationale**: 满足开源协作与透明，避免许可证污染。

**Alternatives considered**: 无（合规为硬约束）。

---

**研究结论**：plan.md 中的 NEEDS CLARIFICATION 均已解决，关键风险（WebView 跨平台差异、会话加密、平台脆弱性、
许可证）均有明确决策与降级/局部化策略。可进入 Phase 1 设计。
