//! 跨平台 WebView 桥接抽象（research R1/R2，章程原则 III）。
//!
//! 抽象 [`PlatformBridge`] 把"打开登录页 / 在平台上下文执行 JS / 关闭"封装为稳定接口，
//! adapter 只产出 JS、由桥接执行，从而复用 WebView 持有的平台登录态。
//! 生产实现封装 WebView2(Win) / WKWebView(macOS) / WebKitGTK(Linux) 差异；
//! Linux 能力受限处显式返回降级错误，禁止静默失败。

use std::collections::HashMap;
use std::sync::mpsc::{channel, Sender};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use crate::adapters::PlatformId;
use crate::error::{AppError, AppResult};

/// 注入 JS 执行后由页面回传的结果（经 `report_eval_result` 命令送达）。
pub struct EvalOutcome {
    pub ok: bool,
    pub value: serde_json::Value,
    pub error: Option<String>,
}

/// token → 等待中的 eval 发送端。包装 JS 执行完通过 `report_eval_result` 命令带 token 回传。
fn registry() -> &'static Mutex<HashMap<String, Sender<EvalOutcome>>> {
    static R: OnceLock<Mutex<HashMap<String, Sender<EvalOutcome>>>> = OnceLock::new();
    R.get_or_init(|| Mutex::new(HashMap::new()))
}

/// 由 `report_eval_result` 命令调用：把页面回传的结果送达等待中的 eval（FR：IPC 回传接线）。
pub fn deliver_eval_result(token: &str, outcome: EvalOutcome) {
    if let Some(tx) = registry().lock().expect("eval registry").remove(token) {
        let _ = tx.send(outcome);
    }
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

/// 包装 adapter 注入 JS：执行（await）其返回值，再经 `window.__TAURI__.core.invoke`
/// 带 token 回传给 `report_eval_result` 命令。依赖 `app.withGlobalTauri = true`。
fn wrap_js(token: &str, inner: &str) -> String {
    let token_js = serde_json::to_string(token).unwrap_or_else(|_| "\"\"".into());
    format!(
        r#"
    (async () => {{
      const __t = {token_js};
      const __send = (p) => {{
        try {{
          window.__TAURI__.core.invoke('report_eval_result', {{
            token: __t, ok: !!p.ok, value: p.value === undefined ? null : p.value, error: p.error || null
          }});
        }} catch (e) {{ /* IPC 不可用时无能为力，eval 端会超时 */ }}
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
            eprintln!(
                "[publish-login] open_login ENTER platform={} url={login_url}",
                platform.as_str()
            );
            let label = Self::label(platform);
            if let Some(w) = self.app.get_webview_window(&label) {
                eprintln!("[publish-login] window already exists, focusing");
                let _ = w.show();
                let _ = w.set_focus();
                return Ok(());
            }
            let url = login_url
                .parse()
                .map_err(|e| AppError::Invalid(format!("登录 URL 非法: {e}")))?;

            // 重要：WebView2 创建是异步的，build() 需主线程事件循环继续转动才能完成。
            // 因此本方法（及其调用方 connect_platform）必须运行在【非主线程】，
            // 否则事件循环被占死，build() 永不返回（空白窗口、整窗卡死）。
            // connect_platform 已改为 async 命令 → 在 worker 线程执行，此处可直接 build()。
            eprintln!("[publish-login] building window (off main thread)");
            WebviewWindowBuilder::new(&self.app, &label, WebviewUrl::External(url))
                .title(format!("登录 - {}", platform.as_str()))
                .inner_size(960.0, 720.0)
                .initialization_script(POPUP_REDIRECT_JS)
                .on_navigation(|u| {
                    eprintln!("[publish-login] navigation -> {u}");
                    true
                })
                .on_page_load(|_w, payload| {
                    eprintln!(
                        "[publish-login] page_load {:?} url={}",
                        payload.event(),
                        payload.url()
                    );
                })
                .build()
                .map_err(|e| AppError::Platform(format!("无法创建登录窗口: {e}")))?;
            eprintln!("[publish-login] window BUILT ok");
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

            // token + 一次性通道：包装 JS 在页面执行后经 report_eval_result 命令带 token 回传。
            let token = next_token();
            let (tx, rx) = channel::<EvalOutcome>();
            registry()
                .lock()
                .expect("eval registry")
                .insert(token.clone(), tx);

            if let Err(e) = webview.eval(&wrap_js(&token, js)) {
                registry().lock().expect("eval registry").remove(&token);
                return Err(AppError::Platform(format!("注入 JS 失败: {e}")));
            }

            // 阻塞等待页面回传。注意：本方法只应在 spawn_blocking 线程内调用，避免阻塞主线程事件循环。
            match rx.recv_timeout(Duration::from_secs(30)) {
                Ok(outcome) if outcome.ok => Ok(outcome.value),
                Ok(outcome) => Err(AppError::Platform(
                    outcome.error.unwrap_or_else(|| "平台执行失败".into()),
                )),
                Err(_) => {
                    registry().lock().expect("eval registry").remove(&token);
                    Err(AppError::Network("WebView 执行超时（30s）".into()))
                }
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
