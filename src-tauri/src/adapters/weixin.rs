//! 微信公众号适配器。样式保真要求最高（SC-003）：直接承载 doocs/md 的内联样式 HTML。
//!
//! 注意：下列注入 JS 中的内部端点/选择子需在真实公众号编辑页经验核验后定稿（research R1/R6/R7）。
//! 当前为结构正确、约定一致的最佳努力模板，标注 `// TODO(empirical)` 处需联调。

use crate::adapters::{PlatformId, PublishAdapter};

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
            const el = document.querySelector(
              '.weui-desktop-account__nickname, .account_nickname, .weui-desktop-account__info strong'
            );
            if (el && el.textContent) account = el.textContent.trim();
            return { loggedIn, account, href };
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
        // TODO(empirical): 端点、token、字段名与返回结构需联调。
        format!(
            r#"
        (async () => {{
          try {{
            const b64 = "{image_b64}";
            const bin = atob(b64); const arr = new Uint8Array(bin.length);
            for (let i=0;i<bin.length;i++) arr[i]=bin.charCodeAt(i);
            const fd = new FormData();
            fd.append('file', new Blob([arr]), {filename:?});
            const r = await fetch('/cgi-bin/filetransfer?action=upload_material&type=image', {{ method:'POST', body: fd, credentials:'include' }});
            const j = await r.json();
            if (j && j.content_url) return {{ ok:true, url:j.content_url }};
            return {{ ok:false, error: JSON.stringify(j) }};
          }} catch (e) {{ return {{ ok:false, error:String(e) }}; }}
        }})()
        "#,
            image_b64 = image_b64,
            filename = filename
        )
    }

    fn save_draft_js(&self, title: &str, html: &str) -> String {
        // 仅调用「保存草稿」，绝不触发群发/发布（FR-008）；每次新建（FR-016a）。
        // TODO(empirical): draft/add 端点、token 与字段需联调。
        let title_js = serde_json::to_string(title).unwrap_or_else(|_| "\"\"".into());
        let html_js = serde_json::to_string(html).unwrap_or_else(|_| "\"\"".into());
        format!(
            r#"
        (async () => {{
          try {{
            const title = {title_js}; const content = {html_js};
            const fd = new URLSearchParams();
            fd.append('title', title); fd.append('content', content);
            const r = await fetch('/cgi-bin/operate_appmsg?action=submit&sub=create', {{ method:'POST', body: fd, credentials:'include' }});
            const j = await r.json();
            if (j && (j.ret === 0 || j.base_resp?.ret === 0)) return {{ ok:true, draftId: j.appMsgId || null, url: null }};
            return {{ ok:false, error: JSON.stringify(j) }};
          }} catch (e) {{ return {{ ok:false, error:String(e) }}; }}
        }})()
        "#
        )
    }
}
