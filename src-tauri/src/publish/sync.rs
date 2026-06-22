//! 同步编排（FR-007/008/010a/013/015/016/016a，research R5-R8）。平台无关：经
//! [`PublishAdapter`] 产 JS、[`PlatformBridge`] 执行。图片"全有或全无"，单平台失败隔离。

use base64::Engine;
use chrono::Utc;

use crate::adapters::{PlatformId, PublishAdapter};
use crate::error::{AppError, AppResult};
use crate::publish::webview::PlatformBridge;
use crate::publish::{DraftRef, SyncJob, SyncRequest, SyncStatus};

/// 图片加载抽象：给定 img 的 src，本地图片返回 `(文件名, 字节)`，远程/非本地返回 `None`。
/// 注入此抽象使编排可脱离文件系统单测（research R6）。
pub trait ImageLoader: Send + Sync {
    fn load(&self, src: &str) -> AppResult<Option<(String, Vec<u8>)>>;
}

/// 执行单个同步任务：平台化 HTML → 逐图上传替换（全有或全无）→ 新建草稿。
pub fn run_job(
    bridge: &dyn PlatformBridge,
    adapter: &dyn PublishAdapter,
    loader: &dyn ImageLoader,
    article_path: &str,
    title: &str,
    rendered_html: &str,
) -> SyncJob {
    let mut job = SyncJob::pending(article_path, adapter.id());
    job.status = SyncStatus::Running;
    job.started_at = Some(Utc::now().to_rfc3339());

    match try_run(bridge, adapter, loader, title, rendered_html) {
        Ok(draft) => {
            job.status = SyncStatus::Success;
            job.draft_ref = Some(draft);
        }
        Err(e) => {
            job.status = SyncStatus::Failed;
            job.failure_reason = Some(e.to_string());
        }
    }
    job.finished_at = Some(Utc::now().to_rfc3339());
    job
}

fn try_run(
    bridge: &dyn PlatformBridge,
    adapter: &dyn PublishAdapter,
    loader: &dyn ImageLoader,
    title: &str,
    rendered_html: &str,
) -> AppResult<DraftRef> {
    let platform = adapter.id();

    // 同步前确保平台 WebView 已就绪：窗口可能在登录后被关闭、或重启后仅剩会话标记，
    // 此处复用/重开窗口，避免注入时报「窗口未打开」（与界面"已连接"状态对齐）。
    bridge.ensure_ready(platform, adapter.login_url())?;

    // 实时校验登录态（FR-012）：重开窗口后 cookie 可能已失效，停在登录页。
    // 提前判定可给出清晰的"请重新登录"，而非让后续草稿端点返回晦涩错误。
    let probe = bridge.eval(platform, &adapter.probe_login_js())?;
    if !probe.get("loggedIn").and_then(|b| b.as_bool()).unwrap_or(false) {
        return Err(AppError::Auth(format!(
            "{} 登录态已失效，请在登录窗口重新登录后重试",
            platform.as_str()
        )));
    }

    let mut html = adapter.transform_html(rendered_html);

    // 逐图上传并替换（FR-010）。任一失败 → 整篇失败（FR-010a）。
    for src in extract_img_srcs(&html) {
        let Some((filename, bytes)) = loader.load(&src)? else {
            continue; // 远程/非本地图片，跳过
        };
        let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
        let js = adapter.upload_image_js(&b64, &filename);
        let res = bridge.eval(platform, &js)?;
        let url = parse_upload(&res).map_err(|raw| adapter.map_error(&raw))?;
        html = replace_once(&html, &src, &url);
    }

    // 仅新建草稿，不发布（FR-008/016a）。经 adapter 编排：知乎/掘金走默认单步注入，
    // 微信 override 为「导航编辑器页 + JSAPI 填充 + 点保存」（见 weixin::save_draft）。
    let (draft_id, url) = adapter.save_draft(bridge, title, &html)?;
    Ok(DraftRef {
        platform,
        draft_id,
        url,
    })
}

/// 批量同步（FR-013/015）。串行执行、逐平台隔离；每完成一步经 `emit` 推送进度（FR-014）。
pub fn run_batch(
    bridge: &dyn PlatformBridge,
    loader: &dyn ImageLoader,
    req: &SyncRequest,
    get_adapter: impl Fn(PlatformId) -> Box<dyn PublishAdapter>,
    emit: impl Fn(&SyncJob),
) -> Vec<SyncJob> {
    let mut jobs = Vec::with_capacity(req.platforms.len());
    for platform in &req.platforms {
        let adapter = get_adapter(*platform);
        let job = run_job(
            bridge,
            adapter.as_ref(),
            loader,
            &req.article_path,
            &req.title,
            &req.rendered_html,
        );
        emit(&job);
        jobs.push(job);
    }
    jobs
}

// ---- 解析约定返回 ----

fn parse_upload(v: &serde_json::Value) -> Result<String, String> {
    if v.get("ok").and_then(|b| b.as_bool()).unwrap_or(false) {
        if let Some(url) = v.get("url").and_then(|u| u.as_str()) {
            return Ok(url.to_string());
        }
        return Err("上传返回缺少 url".into());
    }
    Err(v
        .get("error")
        .and_then(|e| e.as_str())
        .unwrap_or("图片上传失败")
        .to_string())
}

// ---- HTML 图片 src 提取/替换（轻量，无需正则依赖） ----

/// 提取所有 `<img ... src="...">` 的 src 值（去重，保序）。
fn extract_img_srcs(html: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let bytes = html.as_bytes();
    let mut i = 0usize;
    while let Some(pos) = html[i..].find("src=") {
        let start = i + pos + 4;
        if start >= bytes.len() {
            break;
        }
        let quote = bytes[start] as char;
        if quote == '"' || quote == '\'' {
            if let Some(end_rel) = html[start + 1..].find(quote) {
                let val = &html[start + 1..start + 1 + end_rel];
                if !val.is_empty() && !out.iter().any(|s| s == val) {
                    out.push(val.to_string());
                }
                i = start + 1 + end_rel + 1;
                continue;
            }
        }
        i = start;
    }
    out
}

fn replace_once(html: &str, from: &str, to: &str) -> String {
    html.replacen(from, to, 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::mock::MockAdapter;
    use crate::publish::webview::mock::MockBridge;
    use serde_json::json;

    struct NoImages;
    impl ImageLoader for NoImages {
        fn load(&self, _src: &str) -> AppResult<Option<(String, Vec<u8>)>> {
            Ok(None)
        }
    }

    struct OneLocalImage {
        fail: bool,
    }
    impl ImageLoader for OneLocalImage {
        fn load(&self, src: &str) -> AppResult<Option<(String, Vec<u8>)>> {
            if src.starts_with("assets/") {
                if self.fail {
                    // 模拟本地读取失败前仍返回字节，由上传环节判失败
                }
                Ok(Some(("img.png".into(), vec![1, 2, 3])))
            } else {
                Ok(None)
            }
        }
    }

    fn weixin_mock() -> Box<dyn PublishAdapter> {
        Box::new(MockAdapter {
            id: PlatformId::Weixin,
        })
    }

    #[test]
    fn extract_srcs_works() {
        let html = r#"<p><img src="assets/a.png"><img src='https://x/y.png'></p>"#;
        let srcs = extract_img_srcs(html);
        assert_eq!(srcs, vec!["assets/a.png".to_string(), "https://x/y.png".into()]);
    }

    #[test]
    fn job_success_no_images() {
        let bridge = MockBridge::new();
        bridge.set(PlatformId::Weixin, "PROBE", json!({"loggedIn":true}));
        bridge.set(PlatformId::Weixin, "SAVE", json!({"ok":true,"draftId":"d1","url":"u1"}));
        let job = run_job(
            &bridge,
            weixin_mock().as_ref(),
            &NoImages,
            "a.md",
            "标题",
            "<p>hi</p>",
        );
        assert_eq!(job.status, SyncStatus::Success);
        assert_eq!(job.draft_ref.unwrap().draft_id.unwrap(), "d1");
    }

    #[test]
    fn job_image_upload_fail_marks_whole_failed() {
        // 上传返回 ok:false → 整篇 Failed，不进入保存（FR-010a/SC-005）
        let bridge = MockBridge::new();
        bridge.set(PlatformId::Weixin, "PROBE", json!({"loggedIn":true}));
        bridge.set(PlatformId::Weixin, "UPLOAD", json!({"ok":false,"error":"413 too large"}));
        bridge.set(PlatformId::Weixin, "SAVE", json!({"ok":true,"draftId":"d1"}));
        let job = run_job(
            &bridge,
            weixin_mock().as_ref(),
            &OneLocalImage { fail: true },
            "a.md",
            "标题",
            r#"<img src="assets/a.png">"#,
        );
        assert_eq!(job.status, SyncStatus::Failed);
        assert!(job.failure_reason.unwrap().contains("413"));
    }

    #[test]
    fn job_image_success_replaces_src_then_saves() {
        let bridge = MockBridge::new();
        bridge.set(PlatformId::Weixin, "PROBE", json!({"loggedIn":true}));
        bridge.set(PlatformId::Weixin, "UPLOAD", json!({"ok":true,"url":"https://mp/img.png"}));
        bridge.set(PlatformId::Weixin, "SAVE", json!({"ok":true,"draftId":"d2"}));
        let job = run_job(
            &bridge,
            weixin_mock().as_ref(),
            &OneLocalImage { fail: false },
            "a.md",
            "标题",
            r#"<img src="assets/a.png">"#,
        );
        assert_eq!(job.status, SyncStatus::Success);
    }

    #[test]
    fn batch_isolates_failures() {
        // 微信成功、知乎失败（未配置响应）→ 互不影响（FR-015）
        let bridge = MockBridge::new();
        bridge.set(PlatformId::Weixin, "PROBE", json!({"loggedIn":true}));
        bridge.set(PlatformId::Weixin, "SAVE", json!({"ok":true,"draftId":"d1"}));
        let req = SyncRequest {
            article_path: "a.md".into(),
            rendered_html: "<p>x</p>".into(),
            title: "t".into(),
            platforms: vec![PlatformId::Weixin, PlatformId::Zhihu],
        };
        let jobs = run_batch(
            &bridge,
            &NoImages,
            &req,
            |p| Box::new(MockAdapter { id: p }),
            |_| {},
        );
        assert_eq!(jobs.len(), 2);
        assert_eq!(jobs[0].status, SyncStatus::Success);
        assert_eq!(jobs[1].status, SyncStatus::Failed);
    }

    #[test]
    fn retry_creates_new_job_each_time() {
        // 重复执行各自独立、均新建（FR-016a）：两次 job id 不同
        let bridge = MockBridge::new();
        bridge.set(PlatformId::Weixin, "PROBE", json!({"loggedIn":true}));
        bridge.set(PlatformId::Weixin, "SAVE", json!({"ok":true,"draftId":"d1"}));
        let j1 = run_job(&bridge, weixin_mock().as_ref(), &NoImages, "a.md", "t", "<p/>");
        let j2 = run_job(&bridge, weixin_mock().as_ref(), &NoImages, "a.md", "t", "<p/>");
        assert_ne!(j1.id, j2.id);
    }
}
