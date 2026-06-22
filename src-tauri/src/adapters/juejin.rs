//! 掘金适配器。再提供一种平台形态以验证适配器解耦（章程原则 II）。
//! 注入 JS 端点/字段需经验核验，标注 `// TODO(empirical)`。

use crate::adapters::{PlatformId, PublishAdapter};

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
        // TODO(empirical): 校验 user_api/get_info 返回结构。
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
        // 掘金以 Markdown 为主，但其草稿接口接受 HTML 内容字段；此处透传，富文本清洗作后续增强。
        base_html.to_string()
    }

    fn upload_image_js(&self, image_b64: &str, filename: &str) -> String {
        // TODO(empirical): 掘金图片上传端点与返回字段联调。
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
            const j = await r.json();
            if (j && j.data && j.data.url) return {{ ok:true, url: j.data.url }};
            return {{ ok:false, error: JSON.stringify(j) }};
          }} catch (e) {{ return {{ ok:false, error:String(e) }}; }}
        }})()
        "#,
            image_b64 = image_b64,
            filename = filename
        )
    }

    fn save_draft_js(&self, title: &str, html: &str) -> String {
        // 仅创建草稿（FR-008），每次新建（FR-016a）。
        // TODO(empirical): /content_api/v1/article_draft/create 字段联调。
        let title_js = serde_json::to_string(title).unwrap_or_else(|_| "\"\"".into());
        let html_js = serde_json::to_string(html).unwrap_or_else(|_| "\"\"".into());
        format!(
            r#"
        (async () => {{
          try {{
            const r = await fetch('https://api.juejin.cn/content_api/v1/article_draft/create', {{
              method:'POST', credentials:'include',
              headers: {{ 'content-type':'application/json' }},
              body: JSON.stringify({{ title: {title_js}, edit_type: 10, html_content: {html_js} }})
            }});
            const j = await r.json();
            const id = j && j.data && j.data.id;
            if (id) return {{ ok:true, draftId: String(id), url: 'https://juejin.cn/editor/drafts/'+id }};
            return {{ ok:false, error: JSON.stringify(j) }};
          }} catch (e) {{ return {{ ok:false, error:String(e) }}; }}
        }})()
        "#
        )
    }
}
