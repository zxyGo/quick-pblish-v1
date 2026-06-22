//! 知乎适配器。走编辑器 UI 自动化（导航「写文章」页 → 注入填标题 + 粘贴 Markdown → 触发
//! 知乎「确认并解析」弹窗），而非直接调内部文章接口——后者撞 csrf/签名、字段易变，已弃用，
//! 与微信适配器同思路（research R7）。方案参考 doocs/md 浏览器扩展（cose）知乎填充实现。
//!
//! 知乎现代编辑器（DraftEditor）支持「粘贴 Markdown → 弹窗确认并解析」转富文本，故以
//! [`DraftMeta::markdown`] 为内容源（本地图片已由 sync 编排预上传替换为知乎可访问 URL）。

use crate::adapters::{DraftMeta, DraftOutcome, PlatformId, PublishAdapter};
use crate::error::{AppError, AppResult};
use crate::publish::webview::PlatformBridge;

/// 在「写文章」页填标题、把 Markdown 以粘贴事件送入 DraftEditor，并点掉「确认并解析」/「确认」弹窗。
/// `__TITLE_JSON__` / `__MARKDOWN_JSON__` 在 Rust 侧用 serde_json 字面量替换注入。
/// 关键：用 `ClipboardEvent('paste')` 注入纯文本 Markdown 才能触发知乎的 Markdown 检测弹窗，
/// `execCommand('insertText')` 不会触发（cose 验证结论）。
/// 返回 `{"ok":true,"draftId":string|null,"url":string|null}` 或 `{"ok":false,"error":string}`。
const ZHIHU_FILL_JS: &str = r#"
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
  const clickButton = async (matcher, timeout) => {
    const start = Date.now();
    while (Date.now() - start < timeout) {
      for (const btn of document.querySelectorAll('button')) {
        if (matcher((btn.textContent || '').trim())) { btn.click(); return true; }
      }
      await sleep(200);
    }
    return false;
  };
  const draftUrl = () => {
    const m = location.pathname.match(/\/p\/(\d+)/);
    return m ? { draftId: m[1], url: 'https://zhuanlan.zhihu.com/p/' + m[1] + '/edit' } : { draftId: null, url: location.href };
  };
  try {
    const title = __TITLE_JSON__;
    const markdown = __MARKDOWN_JSON__;

    // 等编辑器初始化（避免「草稿加载中」覆盖输入）。
    await sleep(1500);

    // 1. 标题：textarea，用原生 setter 写值以让 React 识别变更。
    if (title) {
      const titleInput = await waitFor('textarea[placeholder*="标题"]', 12000);
      if (titleInput) {
        titleInput.focus();
        const setter = (Object.getOwnPropertyDescriptor(window.HTMLTextAreaElement.prototype, 'value') || {}).set;
        if (setter) setter.call(titleInput, title); else titleInput.value = title;
        titleInput.dispatchEvent(new Event('input', { bubbles: true }));
        titleInput.dispatchEvent(new Event('change', { bubbles: true }));
      }
      await sleep(400);
    }

    // 2. 找到并激活正文 DraftEditor。
    let editor = null;
    for (const sel of ['.public-DraftEditor-content', '[contenteditable="true"]', '.DraftEditor-root']) {
      editor = document.querySelector(sel);
      if (editor) break;
    }
    if (!editor) return { ok: false, error: '未找到知乎正文编辑器' };
    const rect = editor.getBoundingClientRect();
    const cx = rect.left + rect.width / 2, cy = rect.top + rect.height / 2;
    for (const type of ['mousedown', 'mouseup', 'click']) {
      editor.dispatchEvent(new MouseEvent(type, { bubbles: true, cancelable: true, view: window, clientX: cx, clientY: cy, button: 0 }));
    }
    editor.focus();
    document.execCommand('selectAll', false);
    document.execCommand('delete', false);
    await sleep(100);

    // 3. 以粘贴事件注入 Markdown 纯文本，触发知乎「确认并解析」弹窗。
    if (markdown) {
      if (typeof DataTransfer === 'undefined' || typeof ClipboardEvent === 'undefined') {
        return { ok: false, error: '浏览器不支持 DataTransfer/ClipboardEvent' };
      }
      const dt = new DataTransfer();
      dt.setData('text/plain', markdown);
      editor.focus();
      editor.dispatchEvent(new ClipboardEvent('paste', { bubbles: true, cancelable: true, clipboardData: dt }));
      await sleep(500);

      // 4. 弹窗：先「确认并解析」，再「确认」。无弹窗（纯文本无 Markdown 语法）则跳过。
      const parsed = await clickButton(t => t.includes('确认并解析'), 5000);
      if (parsed) {
        await sleep(500);
        await clickButton(t => t === '确认', 5000);
      }
      await sleep(800);
    }

    // 解析后知乎自动保存草稿并把 URL 切到 /p/{id}；等它出现，便于 Rust 侧随后精确刷新重载正文。
    for (let i = 0; i < 40 && !/\/p\/\d+/.test(location.pathname); i++) { await sleep(200); }

    const ref = draftUrl();
    return { ok: true, draftId: ref.draftId, url: ref.url };
  } catch (e) { return { ok: false, error: String(e) }; }
})()
"#;

pub struct ZhihuAdapter;

impl ZhihuAdapter {
    pub fn new() -> Self {
        ZhihuAdapter
    }
}

impl PublishAdapter for ZhihuAdapter {
    fn id(&self) -> PlatformId {
        PlatformId::Zhihu
    }

    fn login_url(&self) -> &str {
        "https://www.zhihu.com/signin"
    }

    fn probe_login_js(&self) -> String {
        // 知乎登录态以 /api/v4/me 是否返回带 id 的用户对象判定；账号名尽力读取。
        r#"
        (async () => {
          try {
            const r = await fetch('/api/v4/me', { credentials: 'include' });
            if (!r.ok) return { loggedIn:false, account:null };
            const j = await r.json();
            return { loggedIn: !!j.id, account: j.name || null };
          } catch (e) { return { loggedIn:false, account:null }; }
        })()
        "#
        .to_string()
    }

    fn transform_html(&self, base_html: &str) -> String {
        // 知乎走 Markdown 粘贴方案，HTML 仅用于 sync 编排提取/上传本地图片，原样透传即可。
        base_html.to_string()
    }

    fn upload_image_js(&self, image_b64: &str, filename: &str) -> String {
        // 本地图片预上传到知乎图床，换回可访问 URL 后替换进 Markdown（远程图由知乎粘贴时自行转存）。
        // 关键：知乎写接口需带 `x-xsrftoken`（取自 cookie `_xsrf`），否则被拦到登录/错误页返回
        // HTML，`r.json()` 抛 `SyntaxError: Unexpected token '<'`。此处带 xsrf 并先校验响应类型，
        // 非 JSON 时回传 HTTP 状态 + 响应片段，避免裸 SyntaxError 且便于诊断。
        // TODO(empirical): /api/v4/images 字段/返回结构与多步上传流程需抓包核验。
        format!(
            r#"
        (async () => {{
          try {{
            const xsrf = (document.cookie.match(/_xsrf=([^;]+)/) || [])[1] || '';
            const b64 = "{image_b64}";
            const bin = atob(b64); const arr = new Uint8Array(bin.length);
            for (let i=0;i<bin.length;i++) arr[i]=bin.charCodeAt(i);
            const fd = new FormData();
            fd.append('file', new Blob([arr]), {filename:?}); fd.append('source','article');
            const headers = {{}};
            if (xsrf) headers['x-xsrftoken'] = decodeURIComponent(xsrf);
            const r = await fetch('https://www.zhihu.com/api/v4/images', {{
              method:'POST', body: fd, credentials:'include', headers
            }});
            const ct = r.headers.get('content-type') || '';
            if (!ct.includes('json')) {{
              const t = await r.text();
              return {{ ok:false, error: '知乎图片上传返回非 JSON（HTTP ' + r.status + '）：' + t.slice(0,160) }};
            }}
            const j = await r.json();
            // 兼容多种返回：直接 url，或 {{image_id}} + 异步处理后取 src。
            const url = j && (j.url || j.original_src || (j.upload_file && j.upload_file.url));
            if (url) return {{ ok:true, url }};
            return {{ ok:false, error: '知乎图片上传未返回 url：' + JSON.stringify(j).slice(0,200) }};
          }} catch (e) {{ return {{ ok:false, error:String(e) }}; }}
        }})()
        "#,
            image_b64 = image_b64,
            filename = filename
        )
    }

    fn save_draft_js(&self, _title: &str, _html: &str) -> String {
        // 知乎走 save_draft override（导航 + UI 自动化），不经此单步注入。保留满足 trait。
        r#"(() => ({ ok: false, error: "zhihu 应走 save_draft 编排（编辑器 UI 自动化），不应调用 save_draft_js" }))()"#
            .to_string()
    }

    /// 知乎新建草稿：导航「写文章」页 → 注入填标题 + 粘贴 Markdown + 点掉解析弹窗。
    /// 不调内部接口，由编辑器自身在粘贴/解析后写草稿（知乎自动保存）。
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
                "知乎草稿缺少 Markdown 正文（请确认前端已传 markdown）".into(),
            ));
        }

        // 1. 导航到「写文章」页（每次新建草稿，FR-016a）。导航会换页，须独立于后续注入。
        bridge.navigate(platform, "https://zhuanlan.zhihu.com/write")?;

        // 2. 注入填充脚本（标题 + Markdown 粘贴 + 解析弹窗）。
        let title_js = serde_json::to_string(title).unwrap_or_else(|_| "\"\"".into());
        let md_js = serde_json::to_string(&meta.markdown).unwrap_or_else(|_| "\"\"".into());
        let fill_js = ZHIHU_FILL_JS
            .replace("__TITLE_JSON__", &title_js)
            .replace("__MARKDOWN_JSON__", &md_js);
        // 编辑器 UI 自动化脚本自身最坏耗时已近 30s（等编辑器挂载 + 两次弹窗轮询），
        // 默认 30s 预算在重型 SPA 慢加载时会顶满，故给 60s。
        let res = bridge.eval_with_timeout(platform, &fill_js, std::time::Duration::from_secs(60))?;

        if res.get("ok").and_then(|b| b.as_bool()).unwrap_or(false) {
            let draft_id = res
                .get("draftId")
                .and_then(|d| d.as_str())
                .map(|s| s.to_string());
            let url = res.get("url").and_then(|u| u.as_str()).map(|s| s.to_string());
            // 知乎解析粘贴后会把正文存入草稿箱，但当前编辑器实例常不刷新渲染（表现为
            // 「草稿备份有正文、编辑页空白」）。对齐 cose 同步后的 `tabs.reload`：等草稿落库后
            // 重新导航到草稿编辑页，让编辑器从已存草稿重载并渲染正文。
            if let Some(u) = url.as_deref() {
                std::thread::sleep(std::time::Duration::from_secs(2));
                let _ = bridge.navigate(platform, u);
            }
            Ok((draft_id, url))
        } else {
            let raw = res
                .get("error")
                .and_then(|e| e.as_str())
                .unwrap_or("知乎草稿保存失败");
            Err(self.map_error(raw))
        }
    }
}
