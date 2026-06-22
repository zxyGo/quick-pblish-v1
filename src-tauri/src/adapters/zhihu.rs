//! 知乎适配器。文章草稿 API 形态与公众号不同，用于验证适配器解耦（章程原则 II）。
//! 注入 JS 端点/字段需经验核验，标注 `// TODO(empirical)`。

use crate::adapters::{PlatformId, PublishAdapter};

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
        // TODO(empirical): 校验 /api/v4/me 字段。
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
        // 知乎编辑器接受较规整的 HTML；此处直接透传，复杂清洗作为后续增强。
        base_html.to_string()
    }

    fn upload_image_js(&self, image_b64: &str, filename: &str) -> String {
        // TODO(empirical): 知乎图片上传走 /api/v4/images，需 sign/字段联调。
        format!(
            r#"
        (async () => {{
          try {{
            const b64 = "{image_b64}";
            const bin = atob(b64); const arr = new Uint8Array(bin.length);
            for (let i=0;i<bin.length;i++) arr[i]=bin.charCodeAt(i);
            const fd = new FormData();
            fd.append('file', new Blob([arr]), {filename:?}); fd.append('source','article');
            const r = await fetch('/api/v4/images', {{ method:'POST', body: fd, credentials:'include' }});
            const j = await r.json();
            if (j && (j.url || j.original_src)) return {{ ok:true, url: j.url || j.original_src }};
            return {{ ok:false, error: JSON.stringify(j) }};
          }} catch (e) {{ return {{ ok:false, error:String(e) }}; }}
        }})()
        "#,
            image_b64 = image_b64,
            filename = filename
        )
    }

    fn save_draft_js(&self, title: &str, html: &str) -> String {
        // 仅存草稿，不发布（FR-008）；每次新建一篇文章草稿（FR-016a）。
        // TODO(empirical): /api/v4/articles 草稿创建端点与字段联调。
        let title_js = serde_json::to_string(title).unwrap_or_else(|_| "\"\"".into());
        let html_js = serde_json::to_string(html).unwrap_or_else(|_| "\"\"".into());
        format!(
            r#"
        (async () => {{
          try {{
            const r = await fetch('/api/v4/articles', {{
              method:'POST', credentials:'include',
              headers: {{ 'content-type':'application/json' }},
              body: JSON.stringify({{ title: {title_js}, content: {html_js}, delta_time: 0 }})
            }});
            const j = await r.json();
            if (j && j.id) return {{ ok:true, draftId: String(j.id), url: 'https://zhuanlan.zhihu.com/p/'+j.id+'/edit' }};
            return {{ ok:false, error: JSON.stringify(j) }};
          }} catch (e) {{ return {{ ok:false, error:String(e) }}; }}
        }})()
        "#
        )
    }
}
