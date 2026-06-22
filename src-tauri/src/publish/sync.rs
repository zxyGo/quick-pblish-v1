//! 同步编排（FR-007/008/010a/013/015/016/016a，research R5-R8）。平台无关：经
//! [`PublishAdapter`] 产 JS、[`PlatformBridge`] 执行。图片"全有或全无"，单平台失败隔离。

use base64::Engine;
use chrono::Utc;

use crate::adapters::{DraftMeta, PlatformId, PublishAdapter};
use crate::error::{AppError, AppResult};
use crate::publish::webview::PlatformBridge;
use crate::publish::{DraftRef, SyncJob, SyncRequest, SyncStatus};

/// 图片加载抽象：给定 img 的 src，本地图片返回 `(文件名, 字节)`，远程/非本地返回 `None`。
/// 注入此抽象使编排可脱离文件系统单测（research R6）。
pub trait ImageLoader: Send + Sync {
    fn load(&self, src: &str) -> AppResult<Option<(String, Vec<u8>)>>;
}

/// 执行单个同步任务：平台化 HTML → 逐图上传替换（全有或全无）→ 新建草稿。
/// `digest`/`cover` 为可选摘要与封面引用，留空时由 [`try_run`] 自动兜底。
#[allow(clippy::too_many_arguments)]
pub fn run_job(
    bridge: &dyn PlatformBridge,
    adapter: &dyn PublishAdapter,
    loader: &dyn ImageLoader,
    article_path: &str,
    title: &str,
    rendered_html: &str,
    digest: Option<&str>,
    cover: Option<&str>,
) -> SyncJob {
    let mut job = SyncJob::pending(article_path, adapter.id());
    job.status = SyncStatus::Running;
    job.started_at = Some(Utc::now().to_rfc3339());

    match try_run(bridge, adapter, loader, title, rendered_html, digest, cover) {
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

#[allow(clippy::too_many_arguments)]
fn try_run(
    bridge: &dyn PlatformBridge,
    adapter: &dyn PublishAdapter,
    loader: &dyn ImageLoader,
    title: &str,
    rendered_html: &str,
    digest: Option<&str>,
    cover: Option<&str>,
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

    // 封面源在「上传前」确定：用户显式指定优先，否则取正文首图。须在替换前取，
    // 否则首图 src 已被换成平台 URL，拿不到本地字节供封面单独上传。
    let cover_src = cover
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .or_else(|| extract_img_srcs(&html).into_iter().next());

    // 逐图上传并替换（FR-010）。任一失败 → 整篇失败（FR-010a）。
    for src in extract_img_srcs(&html) {
        let Some((filename, bytes)) = loader.load(&src)? else {
            continue; // 远程/非本地图片，跳过
        };
        let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
        let js = adapter.upload_image_js(&b64, &filename);
        let res = bridge.eval(platform, &js)?;
        let url = parse_upload(&res).map_err(|raw| adapter.map_error(&raw))?;
        // src 已去重，故把该图的**所有**引用都替换为上传后的 URL（同图多处引用时避免
        // 漏替导致平台端残留本地路径坏图）。
        html = html.replace(&src, &url);
    }

    // 摘要兜底：用户显式指定优先，否则从最终 HTML 文本提取前若干字（微信摘要上限 120）。
    let digest = digest
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| fallback_digest(&html, 120));

    // 封面字节：仅本地图片可加载（远程图 loader 返回 None → 无封面，降级处理）。
    // 封面失败不应整篇失败（草稿允许无封面），故此处吞掉加载错误为 None。
    let cover_bytes = cover_src.and_then(|src| match loader.load(&src) {
        Ok(Some((filename, bytes))) => Some((
            filename,
            base64::engine::general_purpose::STANDARD.encode(&bytes),
        )),
        _ => None,
    });

    let meta = DraftMeta {
        digest,
        cover: cover_bytes,
    };

    // 仅新建草稿，不发布（FR-008/016a）。经 adapter 编排：知乎/掘金走默认单步注入，
    // 微信 override 为「导航编辑器页 + JSAPI 填充 + 点保存」（见 weixin::save_draft）。
    let (draft_id, url) = adapter.save_draft(bridge, title, &html, &meta)?;
    Ok(DraftRef {
        platform,
        draft_id,
        url,
    })
}

/// 从 HTML 提取纯文本摘要兜底：剥标签、压缩空白，取前 `max` 个字符。
fn fallback_digest(html: &str, max: usize) -> String {
    let mut text = String::new();
    let mut in_tag = false;
    for ch in html.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => text.push(ch),
            _ => {}
        }
    }
    // 压缩连续空白为单个空格，去首尾空白。
    let collapsed = text.split_whitespace().collect::<Vec<_>>().join(" ");
    collapsed.chars().take(max).collect()
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
            req.digest.as_deref(),
            req.cover.as_deref(),
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
    fn fallback_digest_strips_tags_and_truncates() {
        let html = r#"<p>Hello <b>world</b></p>  <p>第二段</p>"#;
        assert_eq!(fallback_digest(html, 120), "Hello world 第二段");
        // 截断按字符计（中文不被切坏）
        assert_eq!(fallback_digest("<p>一二三四五</p>", 3), "一二三");
        // 空白压缩 + 去首尾
        assert_eq!(fallback_digest("<p>  a   b  </p>", 120), "a b");
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
            None,
            None,
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
            None,
            None,
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
            None,
            None,
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
            digest: None,
            cover: None,
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
        let j1 = run_job(&bridge, weixin_mock().as_ref(), &NoImages, "a.md", "t", "<p/>", None, None);
        let j2 = run_job(&bridge, weixin_mock().as_ref(), &NoImages, "a.md", "t", "<p/>", None, None);
        assert_ne!(j1.id, j2.id);
    }
}
