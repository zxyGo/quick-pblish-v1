//! 跨平台 WebView 桥接抽象（research R1/R2，章程原则 III）。
//!
//! 抽象 [`PlatformBridge`] 把"打开登录页 / 在平台上下文执行 JS / 关闭"封装为稳定接口，
//! adapter 只产出 JS、由桥接执行，从而复用 WebView 持有的平台登录态。
//! 生产实现封装 WebView2(Win) / WKWebView(macOS) / WebKitGTK(Linux) 差异；
//! Linux 能力受限处显式返回降级错误，禁止静默失败。

use std::time::{Duration, Instant};

use base64::Engine;

use crate::adapters::PlatformId;
use crate::error::{AppError, AppResult};

/// 注入 JS 执行后写入 document.title 的结果（由 eval 轮询 title 解析）。
#[derive(serde::Deserialize)]
struct EvalOutcome {
    ok: bool,
    #[serde(default)]
    value: serde_json::Value,
    #[serde(default)]
    error: Option<String>,
}

fn next_token() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{ts:x}{:x}", N.fetch_add(1, Ordering::Relaxed))
}

/// hash 回传协议前缀：`location.hash = __EVAL__<token>__<base64(json)>`。
fn hash_marker(token: &str) -> String {
    format!("__EVAL__{token}__")
}

/// 包装 adapter 注入 JS：执行（await）其返回值后，把结果 JSON（UTF-8 → base64）
/// 写入 `location.hash`，以 `__EVAL__<token>__` 前缀标记，供 Rust 侧轮询 `webview.url()`
/// 的 fragment 读取。
///
/// 为何走 hash 通道：登录复用的是远程平台页面（如 mp.weixin.qq.com），Tauri 出于安全
/// 默认不向远程页面注入 IPC 桥（`window.__TAURI__` 不存在），无法用 invoke 回传；
/// 原生窗口标题被 Tauri 锁定不随 `document.title` 变；而 `location.hash` 变更会即时反映到
/// WebView2 的 Source，可由 `webview.url()` 读取，是不依赖 IPC、无刷新副作用的回传旁路。
/// base64（btoa(UTF-8)）确保中文等非 ASCII 账号名安全通过 URL fragment。
fn wrap_js_hash(token: &str, inner: &str) -> String {
    let token_js = serde_json::to_string(token).unwrap_or_else(|_| "\"\"".into());
    format!(
        r#"
    (async () => {{
      const __t = {token_js};
      const __send = (p) => {{
        try {{
          const json = JSON.stringify({{
            ok: !!p.ok, value: p.value === undefined ? null : p.value, error: p.error || null
          }});
          const b64 = btoa(unescape(encodeURIComponent(json)));
          location.hash = "__EVAL__" + __t + "__" + b64;
        }} catch (e) {{ /* 序列化失败也无能为力，eval 端会超时 */ }}
      }};
      try {{ const v = await ({inner}); __send({{ ok: true, value: v }}); }}
      catch (e) {{ __send({{ ok: false, error: String(e) }}); }}
    }})();
    "#
    )
}

/// 平台 WebView 桥接：登录态承载与 JS 执行。
pub trait PlatformBridge: Send + Sync {
    /// 打开/聚焦该平台登录 WebView（FR-001）。
    fn open_login(&self, platform: PlatformId, login_url: &str) -> AppResult<()>;

    /// 在该平台 WebView 上下文执行 JS 并取回 JSON 结果（约定见各 adapter 注释）。
    fn eval(&self, platform: PlatformId, js: &str) -> AppResult<serde_json::Value>;

    /// 关闭该平台 WebView（断开/结束同步时）。
    fn close(&self, platform: PlatformId) -> AppResult<()>;
}

mod tauri_impl {
    use super::*;
    use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

    /// 登录 WebView 注入脚本：把 `window.open` / `target="_blank"` 触发的新原生窗口
    /// 改为当前窗口内导航。
    ///
    /// 原因：内嵌登录 WebView（WebView2）对页面 `window.open` 会触发 `NewWindowRequested`，
    /// wry 默认弹出的新原生窗口既不加载内容（空白）也无事件处理（无法关闭）。
    /// 微信公众号等平台登录流程会用 `window.open` 跳转，故统一在同一上下文内导航，
    /// 既复用登录态又规避空白卡死窗口。脚本经 `initialization_script` 在每次文档创建前执行，
    /// 跨页面跳转持续生效。
    const POPUP_REDIRECT_JS: &str = r#"
    (function () {
      'use strict';
      try {
        var nav = function (url) {
          if (!url) return;
          try { window.top.location.href = url; }
          catch (e) { window.location.href = url; }
        };
        // 覆盖 window.open：返回当前 window 以兼容调用方后续 .focus()/.location 访问。
        window.open = function (url) { nav(url); return window; };
        // 捕获阶段拦截 target="_blank" 链接点击，改为当前窗口导航。
        document.addEventListener('click', function (e) {
          var a = e.target && e.target.closest ? e.target.closest('a[target="_blank"]') : null;
          if (a && a.href) { e.preventDefault(); nav(a.href); }
        }, true);
      } catch (e) { /* 注入失败不阻断登录页加载 */ }
    })();
    "#;

    /// 生产桥接：每平台一个隐藏/可显 WebView 窗口，标签为 `publish-<platform>`。
    pub struct TauriBridge {
        app: AppHandle,
    }

    impl TauriBridge {
        pub fn new(app: AppHandle) -> Self {
            TauriBridge { app }
        }

        fn label(platform: PlatformId) -> String {
            format!("publish-{}", platform.as_str())
        }
    }

    impl PlatformBridge for TauriBridge {
        fn open_login(&self, platform: PlatformId, login_url: &str) -> AppResult<()> {
            let label = Self::label(platform);
            if let Some(w) = self.app.get_webview_window(&label) {
                // 已有同标签窗口：尝试显示并聚焦复用。
                // 注意：用户手动关闭后，标签可能短暂残留为陈旧句柄，对其 show()/set_focus()
                // 会失败而非真正弹窗。此时清掉旧句柄并继续重建，确保再次点击「登录」能开新窗。
                if w.show().is_ok() && w.set_focus().is_ok() {
                    return Ok(());
                }
                let _ = w.close();
            }
            let url = login_url
                .parse()
                .map_err(|e| AppError::Invalid(format!("登录 URL 非法: {e}")))?;

            // 重要：WebView2 创建是异步的，build() 需主线程事件循环继续转动才能完成。
            // 因此本方法（及其调用方 connect_platform）必须运行在【非主线程】，
            // 否则事件循环被占死，build() 永不返回（空白窗口、整窗卡死）。
            // connect_platform 已改为 async 命令 → 在 worker 线程执行，此处可直接 build()。
            WebviewWindowBuilder::new(&self.app, &label, WebviewUrl::External(url))
                .title(format!("登录 - {}", platform.as_str()))
                .inner_size(960.0, 720.0)
                .initialization_script(POPUP_REDIRECT_JS)
                .build()
                .map_err(|e| AppError::Platform(format!("无法创建登录窗口: {e}")))?;
            Ok(())
        }

        fn eval(&self, platform: PlatformId, js: &str) -> AppResult<serde_json::Value> {
            // 该平台 WebView 必须已打开（连接时创建），否则无登录态上下文可复用。
            let webview = self
                .app
                .get_webview_window(&Self::label(platform))
                .ok_or_else(|| {
                    AppError::Auth(format!("{} 未连接（窗口未打开），请先登录", platform.as_str()))
                })?;

            let token = next_token();
            let marker = hash_marker(&token);

            // 注入：页面执行 JS 后把结果（base64 JSON）写进 location.hash（带本次 token 前缀）。
            if let Err(e) = webview.eval(&wrap_js_hash(&token, js)) {
                return Err(AppError::Platform(format!("注入 JS 失败: {e}")));
            }

            // 轮询 webview.url() 的 fragment 取回结果（不依赖远程页面 IPC）。
            // 注意：本方法只应在 spawn_blocking 线程内调用。
            let deadline = Instant::now() + Duration::from_secs(30);
            let mut last_frag = String::new();
            let outcome = loop {
                if let Ok(url) = webview.url() {
                    let frag = url.fragment().unwrap_or("");
                    if frag != last_frag {
                        eprintln!("[publish-eval] fragment -> {frag}");
                        last_frag = frag.to_string();
                    }
                    if let Some(b64) = url.fragment().and_then(|f| f.strip_prefix(&marker)) {
                        let json = base64::engine::general_purpose::STANDARD
                            .decode(b64)
                            .map_err(|e| AppError::Platform(format!("回传 base64 解码失败: {e}")))
                            .and_then(|bytes| {
                                String::from_utf8(bytes).map_err(|e| {
                                    AppError::Platform(format!("回传 UTF-8 解码失败: {e}"))
                                })
                            })?;
                        match serde_json::from_str::<EvalOutcome>(&json) {
                            Ok(o) => break o,
                            Err(e) => {
                                return Err(AppError::Platform(format!(
                                    "解析平台返回失败: {e}; raw={json}"
                                )))
                            }
                        }
                    }
                }
                if Instant::now() >= deadline {
                    return Err(AppError::Network("WebView 执行超时（30s）".into()));
                }
                std::thread::sleep(Duration::from_millis(100));
            };

            if outcome.ok {
                Ok(outcome.value)
            } else {
                Err(AppError::Platform(
                    outcome.error.unwrap_or_else(|| "平台执行失败".into()),
                ))
            }
        }

        fn close(&self, platform: PlatformId) -> AppResult<()> {
            if let Some(w) = self.app.get_webview_window(&Self::label(platform)) {
                w.close()
                    .map_err(|e| AppError::Platform(format!("关闭窗口失败: {e}")))?;
            }
            Ok(())
        }
    }
}

pub use tauri_impl::TauriBridge;

#[cfg(test)]
pub mod mock {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Mutex;

    /// 测试桥接：按注入 JS 的前缀返回脚本化 JSON，驱动 sync 编排单测，不触网。
    pub struct MockBridge {
        /// key: "<platform>:<js前缀>"，如 "weixin:UPLOAD"；value: 返回 JSON
        pub responses: Mutex<HashMap<String, serde_json::Value>>,
        pub opened: Mutex<Vec<PlatformId>>,
    }

    impl MockBridge {
        pub fn new() -> Self {
            MockBridge {
                responses: Mutex::new(HashMap::new()),
                opened: Mutex::new(Vec::new()),
            }
        }

        pub fn set(&self, platform: PlatformId, js_prefix: &str, value: serde_json::Value) {
            self.responses
                .lock()
                .unwrap()
                .insert(format!("{}:{}", platform.as_str(), js_prefix), value);
        }
    }

    impl PlatformBridge for MockBridge {
        fn open_login(&self, platform: PlatformId, _login_url: &str) -> AppResult<()> {
            self.opened.lock().unwrap().push(platform);
            Ok(())
        }

        fn eval(&self, platform: PlatformId, js: &str) -> AppResult<serde_json::Value> {
            let prefix: String = js.trim().chars().take_while(|c| *c != ':').collect();
            let key = format!("{}:{}", platform.as_str(), prefix.trim());
            self.responses
                .lock()
                .unwrap()
                .get(&key)
                .cloned()
                .ok_or_else(|| AppError::Platform(format!("mock 未配置响应: {key}")))
        }

        fn close(&self, _platform: PlatformId) -> AppResult<()> {
            Ok(())
        }
    }
}
