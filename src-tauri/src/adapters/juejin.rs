//! 掘金适配器。走编辑器 UI 自动化（导航「新建草稿」页 → 注入填标题 + 向 ByteMD/CodeMirror
//! 写入 Markdown），而非直接调内部草稿接口——后者撞 csrf/签名、字段易变，已弃用，与微信/知乎
//! 适配器同思路（research R7）。方案参考 doocs/md 浏览器扩展（cose）掘金填充实现。
//!
//! 掘金编辑器是 ByteMD（基于 CodeMirror）的 Markdown 编辑器，故以 [`DraftMeta::markdown`] 为
//! 内容源（本地图片已由 sync 编排预上传替换为掘金可访问 URL）。

use crate::adapters::{DraftMeta, DraftOutcome, PlatformId, PublishAdapter};
use crate::error::{AppError, AppResult};
use crate::publish::webview::PlatformBridge;

/// 在「新建草稿」页填标题并把 Markdown 写入 ByteMD 的 CodeMirror（降级 textarea）。
/// `__TITLE_JSON__` / `__MARKDOWN_JSON__` 在 Rust 侧用 serde_json 字面量替换注入。
/// 掘金会在内容变化后自动保存草稿；草稿 id 从导航后的 `/editor/drafts/{id}` 路径读取。
/// 返回 `{"ok":true,"draftId":string|null,"url":string|null}` 或 `{"ok":false,"error":string}`。
const JUEJIN_FILL_JS: &str = r#"
(async () => {
  const sleep = (ms) => new Promise(r => setTimeout(r, ms));
  const waitFor = (sel, timeout) => new Promise(res => {
    const found = document.querySelector(sel);
    if (found) return res(found);
    const obs = new MutationObserver(() => {
      const el = document.querySelector(sel);
      if (el) { obs.disconnect(); res(el); }
    });
    obs.observe(document.documentElement, { childList: true, subtree: true });
    setTimeout(() => { obs.disconnect(); res(document.querySelector(sel)); }, timeout);
  });
  const draftUrl = () => {
    const m = location.pathname.match(/\/editor\/drafts\/([^/?#]+)/);
    return m ? { draftId: m[1], url: 'https://juejin.cn/editor/drafts/' + m[1] } : { draftId: null, url: location.href };
  };
  try {
    const title = __TITLE_JSON__;
    const markdown = __MARKDOWN_JSON__;

    // 1. 标题：input，原生 setter 写值确保框架识别。
    if (title) {
      const titleInput = await waitFor('input[placeholder*="标题"]', 12000);
      if (titleInput) {
        titleInput.focus();
        const setter = (Object.getOwnPropertyDescriptor(window.HTMLInputElement.prototype, 'value') || {}).set;
        if (setter) setter.call(titleInput, title); else titleInput.value = title;
        titleInput.dispatchEvent(new Event('input', { bubbles: true }));
        titleInput.dispatchEvent(new Event('change', { bubbles: true }));
      }
    }

    // 2. 等编辑器加载，写入 Markdown。优先 ByteMD 的 CodeMirror 实例 setValue。
    await waitFor('.CodeMirror, .bytemd-body textarea', 12000);
    await sleep(600);
    const cm = document.querySelector('.CodeMirror');
    if (cm && cm.CodeMirror) {
      cm.CodeMirror.setValue(markdown || '');
      cm.CodeMirror.focus();
      await sleep(800);
      const ref = draftUrl();
      return { ok: true, method: 'CodeMirror', draftId: ref.draftId, url: ref.url };
    }
    // 降级：直接写 textarea（部分版本 ByteMD 暴露 textarea）。
    const ta = document.querySelector('.bytemd-body textarea');
    if (ta) {
      ta.focus();
      const setter = (Object.getOwnPropertyDescriptor(window.HTMLTextAreaElement.prototype, 'value') || {}).set;
      if (setter) setter.call(ta, markdown || ''); else ta.value = markdown || '';
      ta.dispatchEvent(new Event('input', { bubbles: true }));
      await sleep(800);
      const ref = draftUrl();
      return { ok: true, method: 'textarea', draftId: ref.draftId, url: ref.url };
    }
    return { ok: false, error: '未找到掘金编辑器（CodeMirror/textarea）' };
  } catch (e) { return { ok: false, error: String(e) }; }
})()
"#;

pub struct JuejinAdapter;

impl JuejinAdapter {
    pub fn new() -> Self {
        JuejinAdapter
    }
}

impl PublishAdapter for JuejinAdapter {
    fn id(&self) -> PlatformId {
        PlatformId::Juejin
    }

    fn login_url(&self) -> &str {
        "https://juejin.cn/login"
    }

    fn probe_login_js(&self) -> String {
        // 掘金登录态以 user_api get_info 是否返回 user_id 判定；账号名尽力读取。
        r#"
        (async () => {
          try {
            const r = await fetch('https://api.juejin.cn/user_api/v1/user/get_info', { credentials:'include' });
            const j = await r.json();
            const u = j && j.data;
            return { loggedIn: !!(u && u.user_id), account: u ? u.user_name : null };
          } catch (e) { return { loggedIn:false, account:null }; }
        })()
        "#
        .to_string()
    }

    fn transform_html(&self, base_html: &str) -> String {
        // 掘金走 Markdown 注入方案，HTML 仅供 sync 编排提取/上传本地图片，原样透传即可。
        base_html.to_string()
    }

    fn upload_image_js(&self, image_b64: &str, filename: &str) -> String {
        // 本地图片预上传到掘金图床，换回 URL 后替换进 Markdown（远程图保持原链）。
        // TODO(empirical): 掘金图片上传端点/返回字段需抓包核验。
        format!(
            r#"
        (async () => {{
          try {{
            const b64 = "{image_b64}";
            const bin = atob(b64); const arr = new Uint8Array(bin.length);
            for (let i=0;i<bin.length;i++) arr[i]=bin.charCodeAt(i);
            const fd = new FormData();
            fd.append('file', new Blob([arr]), {filename:?});
            const r = await fetch('https://api.juejin.cn/imagex/v1/upload', {{ method:'POST', body: fd, credentials:'include' }});
            const ct = r.headers.get('content-type') || '';
            if (!ct.includes('json')) {{
              const t = await r.text();
              return {{ ok:false, error: '掘金图片上传返回非 JSON（HTTP ' + r.status + '）：' + t.slice(0,160) }};
            }}
            const j = await r.json();
            const url = j && j.data && (j.data.url || (j.data.image && j.data.image.url));
            if (url) return {{ ok:true, url }};
            return {{ ok:false, error: '掘金图片上传未返回 url：' + JSON.stringify(j).slice(0,200) }};
          }} catch (e) {{ return {{ ok:false, error:String(e) }}; }}
        }})()
        "#,
            image_b64 = image_b64,
            filename = filename
        )
    }

    fn save_draft_js(&self, _title: &str, _html: &str) -> String {
        // 掘金走 save_draft override（导航 + UI 自动化），不经此单步注入。保留满足 trait。
        r#"(() => ({ ok: false, error: "juejin 应走 save_draft 编排（编辑器 UI 自动化），不应调用 save_draft_js" }))()"#
            .to_string()
    }

    /// 掘金新建草稿：导航「新建草稿」页 → 注入填标题 + CodeMirror.setValue(Markdown)。
    /// 不调内部接口，由编辑器自身在内容变化后自动保存草稿。
    fn save_draft(
        &self,
        bridge: &dyn PlatformBridge,
        title: &str,
        _html: &str,
        meta: &DraftMeta,
    ) -> AppResult<DraftOutcome> {
        let platform = self.id();

        if meta.markdown.trim().is_empty() {
            return Err(AppError::Invalid(
                "掘金草稿缺少 Markdown 正文（请确认前端已传 markdown）".into(),
            ));
        }

        // 1. 导航到「新建草稿」页（每次新建，FR-016a）。导航会换页，须独立于后续注入。
        bridge.navigate(platform, "https://juejin.cn/editor/drafts/new")?;

        // 2. 注入填充脚本（标题 + Markdown 写入 CodeMirror）。
        let title_js = serde_json::to_string(title).unwrap_or_else(|_| "\"\"".into());
        let md_js = serde_json::to_string(&meta.markdown).unwrap_or_else(|_| "\"\"".into());
        let fill_js = JUEJIN_FILL_JS
            .replace("__TITLE_JSON__", &title_js)
            .replace("__MARKDOWN_JSON__", &md_js);
        let res = bridge.eval(platform, &fill_js)?;

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
                .unwrap_or("掘金草稿保存失败");
            Err(self.map_error(raw))
        }
    }
}
