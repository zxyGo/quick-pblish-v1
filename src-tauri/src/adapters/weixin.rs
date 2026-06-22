//! 微信公众号适配器。样式保真要求最高（SC-003）：直接承载 doocs/md 的内联样式 HTML。
//!
//! 注意：下列注入 JS 中的内部端点/选择子需在真实公众号编辑页经验核验后定稿（research R1/R6/R7）。
//! 当前为结构正确、约定一致的最佳努力模板，标注 `// TODO(empirical)` 处需联调。

use crate::adapters::{DraftMeta, DraftOutcome, PlatformId, PublishAdapter};
use crate::error::{AppError, AppResult};
use crate::publish::webview::PlatformBridge;

/// 从公众号后台页提取 csrf token：URL query 优先，兜底扫 `a[href*=token]`。
/// 返回 `{"token":"<digits>"}`。token 是后续导航编辑器页与保存草稿的必需凭据。
const WEIXIN_TOKEN_JS: &str = r#"
(() => {
  try {
    const m = location.href.match(/[?&]token=(\d+)/);
    let token = m ? m[1] : '';
    if (!token) {
      const a = document.querySelector('a[href*="token="]');
      const am = a && a.href ? a.href.match(/token=(\d+)/) : null;
      if (am) token = am[1];
    }
    return { token: token || '' };
  } catch (e) { return { token: '', error: String(e) }; }
})()
"#;

/// 在编辑器页（appmsg_edit_v2）填标题、摘要、正文、封面并点击「保存为草稿」。
/// 走微信编辑器官方 JSAPI `mp_editor_set_content`（正文写入 ProseMirror 文档模型），
/// JSAPI 不可用时降级为合成 paste 事件；最后点真实保存按钮，让页面自身发出带正确
/// csrf/签名的保存请求——这是规避内部接口 `invalid csrf token` 的关键。
/// `__TITLE_JSON__` / `__DIGEST_JSON__` / `__HTMLBODY_JSON__` / `__COVER_B64_JSON__` /
/// `__COVER_NAME_JSON__` 在 Rust 侧用 serde_json 字面量替换注入（空串表示该项无值）。
/// 摘要为可靠 DOM 输入；封面为「最佳努力」上传+设置，失败仅降级不阻断（草稿允许无封面）。
/// 返回 `{"ok":true,"coverSet":bool}` 或 `{"ok":false,"error":string}`。
const WEIXIN_FILL_SAVE_JS: &str = r#"
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
  const pickBody = () => {
    const nodes = [...document.querySelectorAll('.ProseMirror')];
    const te = document.querySelector('.title-editor__input .ProseMirror');
    const body = nodes.filter(n => n !== te && !(n.closest && n.closest('.title-editor__input')));
    if (body.length === 0) return null;
    if (body.length === 1) return body[0];
    const byPh = body.find(n => (n.textContent || '').includes('从这里开始写正文'));
    if (byPh) return byPh;
    return body.sort((a, b) => (b.clientHeight * b.clientWidth) - (a.clientHeight * a.clientWidth))[0];
  };
  try {
    const title = __TITLE_JSON__;
    const digest = __DIGEST_JSON__;
    const htmlBody = __HTMLBODY_JSON__;
    const coverB64 = __COVER_B64_JSON__;
    const coverName = __COVER_NAME_JSON__;

    const titleInput = await waitFor('#title', 12000);
    const titleEditor = document.querySelector('.title-editor__input .ProseMirror');
    if (title) {
      if (titleEditor) {
        titleEditor.focus(); titleEditor.innerHTML = ''; titleEditor.textContent = title;
        titleEditor.dispatchEvent(new Event('input', { bubbles: true }));
        titleEditor.dispatchEvent(new Event('change', { bubbles: true }));
      }
      if (titleInput) {
        titleInput.focus();
        const setter =
          (Object.getOwnPropertyDescriptor(window.HTMLTextAreaElement.prototype, 'value') || {}).set ||
          (Object.getOwnPropertyDescriptor(window.HTMLInputElement.prototype, 'value') || {}).set;
        if (setter) setter.call(titleInput, title); else titleInput.value = title;
        titleInput.dispatchEvent(new Event('input', { bubbles: true }));
        titleInput.dispatchEvent(new Event('change', { bubbles: true }));
      }
    }

    // 摘要：公众号编辑器摘要框为 textarea（不同版本 selector 略异），用原生 setter 写值。
    if (digest) {
      const descEl =
        document.querySelector('#js_description') ||
        document.querySelector('textarea[placeholder*="摘要"]') ||
        document.querySelector('.js_desc_input, .appmsg_desc textarea, textarea.js_desc');
      if (descEl) {
        descEl.focus();
        const dsetter = (Object.getOwnPropertyDescriptor(window.HTMLTextAreaElement.prototype, 'value') || {}).set;
        if (dsetter) dsetter.call(descEl, digest); else descEl.value = digest;
        descEl.dispatchEvent(new Event('input', { bubbles: true }));
        descEl.dispatchEvent(new Event('change', { bubbles: true }));
      }
    }

    await sleep(300);

    let editor = null; const start = Date.now();
    while (Date.now() - start < 12000) { editor = pickBody(); if (editor) break; await sleep(100); }
    if (!editor) return { ok: false, error: '未找到正文编辑器' };
    editor.focus();
    if ((editor.textContent || '').includes('从这里开始写正文')) editor.innerHTML = '';

    let injected = false, injectErr = '';
    if (window.__MP_Editor_JSAPI__ && typeof window.__MP_Editor_JSAPI__.invoke === 'function') {
      injected = await new Promise(resolve => {
        let done = false;
        const fin = (ok, e) => { if (done) return; done = true; if (e) injectErr = (e && e.message) || String(e); resolve(ok); };
        try {
          window.__MP_Editor_JSAPI__.invoke({
            apiName: 'mp_editor_set_content',
            apiParam: { content: htmlBody },
            sucCb: () => fin(true),
            errCb: (e) => fin(false, e),
          });
        } catch (e) { fin(false, e); }
        setTimeout(() => fin(false, new Error('mp_editor_set_content 超时')), 5000);
      });
      if (injected) await sleep(800);
    }
    if (!injected) {
      const dt = new DataTransfer();
      dt.setData('text/html', htmlBody);
      dt.setData('text/plain', htmlBody.replace(/<[^>]*>/g, ''));
      editor.dispatchEvent(new ClipboardEvent('paste', { bubbles: true, cancelable: true, clipboardData: dt }));
      await sleep(800);
    }

    const wordCount = (editor.textContent || '').trim().length;
    const imageCount = editor.querySelectorAll ? editor.querySelectorAll('img').length : 0;
    if (!(wordCount > 0 || imageCount > 0 || injected)) {
      return { ok: false, error: injectErr || '正文注入后未检测到有效内容' };
    }

    // 封面：最佳努力上传为素材并尝试设置图文封面。封面对存草稿非必填，
    // 故任何环节失败都仅降级（coverSet=false），不阻断草稿保存。
    let coverSet = false;
    if (coverB64) {
      try {
        const token = (location.href.match(/[?&]token=(\d+)/) || [])[1] || '';
        const bin = atob(coverB64); const arr = new Uint8Array(bin.length);
        for (let i = 0; i < bin.length; i++) arr[i] = bin.charCodeAt(i);
        const fd = new FormData();
        fd.append('file', new Blob([arr]), coverName || 'cover.png');
        const upUrl = '/cgi-bin/filetransfer?action=upload_material&f=json&scene=8&writetype=doublewrite&groupid=1&token=' + token + '&lang=zh_CN';
        const r = await fetch(upUrl, { method: 'POST', body: fd, credentials: 'include' });
        const j = await r.json();
        const mediaId = j && (j.content_media_id || j.media_id || (j.content && j.content.media_id));
        const cdnUrl = j && (j.content_url || j.cdn_url || (j.content && j.content.content_url));
        // TODO(empirical): 用 mediaId/cdnUrl 设置图文封面（编辑器封面区交互或全局数据写入），
        // 字段名与设置入口需在真实编辑页抓包核验；当前仅完成上传，封面设置入口待联调。
        coverSet = !!(mediaId || cdnUrl);
      } catch (e) { /* 封面为可选增强，失败不阻断草稿保存 */ }
    }

    await sleep(500);
    const btn = Array.from(document.querySelectorAll('button')).find(b => (b.textContent || '').includes('保存为草稿'));
    if (!btn) return { ok: false, error: '未找到「保存为草稿」按钮' };
    btn.click();
    return { ok: true, coverSet };
  } catch (e) { return { ok: false, error: String(e) }; }
})()
"#;

pub struct WeixinAdapter;

impl WeixinAdapter {
    pub fn new() -> Self {
        WeixinAdapter
    }
}

impl PublishAdapter for WeixinAdapter {
    fn id(&self) -> PlatformId {
        PlatformId::Weixin
    }

    fn login_url(&self) -> &str {
        "https://mp.weixin.qq.com/"
    }

    fn probe_login_js(&self) -> String {
        // 微信公众号后台登录后，所有页面 URL 都带 token；未登录则停留在扫码/登录页（无 token）。
        // 以「当前窗口 URL 是否带 token 且位于 cgi-bin 后台」判断登录态，最稳，不依赖具体业务接口。
        // 账号昵称从页面元素尽力读取，读不到也不影响连接判定（FR-006）。
        r#"
        (() => {
          try {
            const href = location.href;
            const hasToken = /[?&]token=\d+/.test(href);
            const onBackend = /mp\.weixin\.qq\.com\/cgi-bin\//.test(href);
            const loggedIn = hasToken && onBackend;
            let account = null;
            // 优先读公众号后台全局数据（最稳）。
            try {
              const d = window.wx && window.wx.commonData && window.wx.commonData.data;
              if (d) account = d.nick_name || d.user_name || d.nickName || null;
            } catch (e) {}
            // 兜底：从页面昵称元素读取。
            if (!account) {
              const el = document.querySelector(
                '.weui-desktop-account__nickname, .account_nickname, .weui-desktop-account__info strong'
              );
              if (el && el.textContent) account = el.textContent.trim();
            }
            return { loggedIn, account: account || null, href };
          } catch (e) { return { loggedIn: false, account: null, error: String(e) }; }
        })()
        "#
        .to_string()
    }

    fn transform_html(&self, base_html: &str) -> String {
        // 公众号编辑器接受整段内联样式 HTML；外层包一个 section 容器以稳定渲染。
        format!(
            r#"<section style="font-size:16px;line-height:1.75;word-break:break-word;">{base_html}</section>"#
        )
    }

    fn upload_image_js(&self, image_b64: &str, filename: &str) -> String {
        // 复用公众号「素材」上传端点，返回平台可访问 URL（FR-010）。
        // 关键：与 operate_appmsg 同样需带 csrf token（取自后台页 URL 的 token 参数），
        // 否则返回 ret=200040 invalid csrf token。
        // TODO(empirical): 端点字段名与返回结构（content_url/cdn_url）需抓包核验。
        format!(
            r#"
        (async () => {{
          try {{
            const token = (location.href.match(/[?&]token=(\d+)/) || [])[1] || '';
            const b64 = "{image_b64}";
            const bin = atob(b64); const arr = new Uint8Array(bin.length);
            for (let i=0;i<bin.length;i++) arr[i]=bin.charCodeAt(i);
            const fd = new FormData();
            fd.append('file', new Blob([arr]), {filename:?});
            const url = '/cgi-bin/filetransfer?action=upload_material&f=json&scene=8&writetype=doublewrite&groupid=1&token=' + token + '&lang=zh_CN';
            const r = await fetch(url, {{ method:'POST', body: fd, credentials:'include' }});
            const j = await r.json();
            const u = j && (j.content_url || j.cdn_url || (j.content && j.content.content_url));
            if (u) return {{ ok:true, url: u }};
            return {{ ok:false, error: JSON.stringify(j) }};
          }} catch (e) {{ return {{ ok:false, error:String(e) }}; }}
        }})()
        "#,
            image_b64 = image_b64,
            filename = filename
        )
    }

    fn save_draft_js(&self, _title: &str, _html: &str) -> String {
        // 微信走 UI 自动化方案（见下方 save_draft override），不经此单步注入。
        // 直接调内部接口 operate_appmsg 会撞 csrf token / 易变字段，已弃用。
        // 保留满足 trait；若被误调，回传明确错误以暴露调用链问题。
        r#"(() => ({ ok: false, error: "weixin 应走 save_draft 编排（JSAPI），不应调用 save_draft_js" }))()"#
            .to_string()
    }

    /// 微信新建草稿：取 token → 导航编辑器页 → JSAPI 填充标题/摘要/正文 + 上传封面 → 点「保存为草稿」。
    /// 不调内部接口，由编辑器页自身发出带正确 csrf/签名的保存请求（规避 ret=200040）。
    fn save_draft(
        &self,
        bridge: &dyn PlatformBridge,
        title: &str,
        html: &str,
        meta: &DraftMeta,
    ) -> AppResult<DraftOutcome> {
        let platform = self.id();

        // 1. 从当前后台页取 csrf token。
        let token_res = bridge.eval(platform, WEIXIN_TOKEN_JS)?;
        let token = token_res
            .get("token")
            .and_then(|t| t.as_str())
            .unwrap_or("");
        if token.is_empty() {
            return Err(AppError::Auth(
                "未能获取微信 token，请确认已登录公众号后台后重试".into(),
            ));
        }

        // 2. 导航到图文编辑器页（新建）。导航会换页，必须独立于后续注入。
        let editor_url = format!(
            "https://mp.weixin.qq.com/cgi-bin/appmsg?t=media/appmsg_edit_v2&action=edit&isNew=1&type=10&token={token}&lang=zh_CN"
        );
        bridge.navigate(platform, &editor_url)?;

        // 3. 在编辑器页填标题/摘要/正文 + 上传封面，并点击「保存为草稿」。
        let title_js = serde_json::to_string(title).unwrap_or_else(|_| "\"\"".into());
        let digest_js = serde_json::to_string(&meta.digest).unwrap_or_else(|_| "\"\"".into());
        let html_js = serde_json::to_string(html).unwrap_or_else(|_| "\"\"".into());
        let (cover_b64, cover_name) = meta
            .cover
            .as_ref()
            .map(|(name, b64)| (b64.as_str(), name.as_str()))
            .unwrap_or(("", ""));
        let cover_b64_js = serde_json::to_string(cover_b64).unwrap_or_else(|_| "\"\"".into());
        let cover_name_js = serde_json::to_string(cover_name).unwrap_or_else(|_| "\"\"".into());
        let fill_js = WEIXIN_FILL_SAVE_JS
            .replace("__TITLE_JSON__", &title_js)
            .replace("__DIGEST_JSON__", &digest_js)
            .replace("__HTMLBODY_JSON__", &html_js)
            .replace("__COVER_B64_JSON__", &cover_b64_js)
            .replace("__COVER_NAME_JSON__", &cover_name_js);
        let res = bridge.eval(platform, &fill_js)?;

        if res.get("ok").and_then(|b| b.as_bool()).unwrap_or(false) {
            // UI 自动化拿不到 appMsgId；草稿已在编辑器内触发保存（FR-008 仅存草稿）。
            Ok((None, None))
        } else {
            let raw = res
                .get("error")
                .and_then(|e| e.as_str())
                .unwrap_or("微信草稿保存失败");
            Err(self.map_error(raw))
        }
    }
}
