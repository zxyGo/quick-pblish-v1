//! 002-multi-platform-publish：平台适配器（章程原则 II「平台适配器解耦」首次落地）。
//!
//! 每个平台实现 [`PublishAdapter`]，互不依赖。新增平台只需：
//! 1) 在 [`PlatformId`] 增加成员；2) 新增 `adapters/<platform>.rs`；3) 在 [`adapter_for`] 注册。
//! 核心编辑/文件/文章管理代码无需改动（FR-017 / SC-006）。

mod juejin;
mod weixin;
mod zhihu;

use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};
use crate::publish::webview::PlatformBridge;

/// 新建草稿结果：`(draft_id, url)`，二者均可为空（如 UI 自动化方案拿不到 appMsgId）。
pub type DraftOutcome = (Option<String>, Option<String>);

/// 受支持平台标识（MVP：公众号 / 知乎 / 掘金）。序列化为小写串以匹配前端契约。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PlatformId {
    Weixin,
    Zhihu,
    Juejin,
}

impl PlatformId {
    pub fn all() -> [PlatformId; 3] {
        [PlatformId::Weixin, PlatformId::Zhihu, PlatformId::Juejin]
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            PlatformId::Weixin => "weixin",
            PlatformId::Zhihu => "zhihu",
            PlatformId::Juejin => "juejin",
        }
    }
}

impl std::str::FromStr for PlatformId {
    type Err = AppError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "weixin" => Ok(PlatformId::Weixin),
            "zhihu" => Ok(PlatformId::Zhihu),
            "juejin" => Ok(PlatformId::Juejin),
            other => Err(AppError::Invalid(format!("未知平台: {other}"))),
        }
    }
}

/// 平台发布能力契约（data-model.md「PublishAdapter」）。
///
/// 适配器只产出"在平台 WebView 上下文执行的 JS"与"平台化 HTML"，不直接触网；
/// 实际执行由 [`crate::publish::webview::PlatformBridge`] 完成，从而：
/// - 复用用户在该 WebView 中的登录态（research R1）；
/// - 平台细节（选择子/内部端点）内聚在各 adapter，便于按平台独立维护与替换。
pub trait PublishAdapter: Send + Sync {
    fn id(&self) -> PlatformId;

    /// 登录页 URL（供 WebView 打开，FR-001）。
    fn login_url(&self) -> &str;

    /// 探测登录态/账号标识的注入 JS（FR-006 / research R4）。
    /// 约定返回：`{"loggedIn":bool,"account":string|null}`。
    fn probe_login_js(&self) -> String;

    /// 把 doocs/md 渲染的内联样式 HTML 平台化（FR-009/011 / research R5）。
    fn transform_html(&self, base_html: &str) -> String;

    /// 复用会话上传单张图片的注入 JS（FR-010 / research R6）。
    /// 约定返回：`{"ok":true,"url":string}` 或 `{"ok":false,"error":string}`。
    fn upload_image_js(&self, image_b64: &str, filename: &str) -> String;

    /// 复用会话"新建草稿"的注入 JS（FR-008/016a / research R7）。
    /// 约定返回：`{"ok":true,"draftId":string|null,"url":string|null}` 或 `{"ok":false,"error":string}`。
    /// 仅供默认 [`save_draft`](PublishAdapter::save_draft) 单步调用；走多步 UI 自动化的平台
    /// （如微信）会 override `save_draft`，此方法对其不再被调用。
    fn save_draft_js(&self, title: &str, html: &str) -> String;

    /// 新建草稿的完整编排（FR-008/016a）。默认实现为「单步注入 `save_draft_js` 并解析回传」，
    /// 适用于可直接调内部接口的平台（知乎/掘金）。
    ///
    /// 需要「导航到编辑器页 + 多步注入」的平台（如微信走 `mp_editor_set_content` JSAPI）
    /// 应 override 本方法，自行用 `bridge` 编排导航与多次 `eval`——因为页面导航会销毁
    /// 注入脚本上下文与 hash 回传通道，无法塞进单段 JS。
    fn save_draft(
        &self,
        bridge: &dyn PlatformBridge,
        title: &str,
        html: &str,
    ) -> AppResult<DraftOutcome> {
        let res = bridge.eval(self.id(), &self.save_draft_js(title, html))?;
        if res.get("ok").and_then(|b| b.as_bool()).unwrap_or(false) {
            let draft_id = res
                .get("draftId")
                .and_then(|d| d.as_str())
                .map(|s| s.to_string());
            let url = res.get("url").and_then(|u| u.as_str()).map(|s| s.to_string());
            Ok((draft_id, url))
        } else {
            let raw = res
                .get("error")
                .and_then(|e| e.as_str())
                .unwrap_or("保存草稿失败");
            Err(self.map_error(raw))
        }
    }

    /// 平台原始错误串 → 统一 [`AppError`]。
    fn map_error(&self, raw: &str) -> AppError {
        AppError::Platform(format!("{}: {raw}", self.id().as_str()))
    }
}

/// 平台 → 适配器实例（注册表，FR-017）。
pub fn adapter_for(platform: PlatformId) -> Box<dyn PublishAdapter> {
    match platform {
        PlatformId::Weixin => Box::new(weixin::WeixinAdapter::new()),
        PlatformId::Zhihu => Box::new(zhihu::ZhihuAdapter::new()),
        PlatformId::Juejin => Box::new(juejin::JuejinAdapter::new()),
    }
}

#[cfg(test)]
pub(crate) mod mock {
    use super::*;

    /// 测试用适配器：不依赖任何真实平台，便于 sync 编排单测。
    pub struct MockAdapter {
        pub id: PlatformId,
    }

    impl PublishAdapter for MockAdapter {
        fn id(&self) -> PlatformId {
            self.id
        }
        fn login_url(&self) -> &str {
            "https://example.test/login"
        }
        fn probe_login_js(&self) -> String {
            "PROBE".into()
        }
        fn transform_html(&self, base_html: &str) -> String {
            format!("<mock>{base_html}</mock>")
        }
        fn upload_image_js(&self, _image_b64: &str, filename: &str) -> String {
            format!("UPLOAD:{filename}")
        }
        fn save_draft_js(&self, title: &str, _html: &str) -> String {
            format!("SAVE:{title}")
        }
    }

    #[test]
    fn platform_id_roundtrip() {
        for p in PlatformId::all() {
            let s = p.as_str();
            assert_eq!(s.parse::<PlatformId>().unwrap(), p);
        }
    }
}
