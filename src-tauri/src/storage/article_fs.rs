use std::path::{Path, PathBuf};

use chrono::Utc;

use crate::domain::ArticleContent;
use crate::error::{AppError, AppResult};

use super::frontmatter::{self, FrontMatter};

/// 将工作目录内相对路径解析为绝对路径，并阻止越界（路径穿越）。
pub fn resolve_in_workspace(root: &Path, relative: &str) -> AppResult<PathBuf> {
    let rel = relative.replace('\\', "/");
    if rel.split('/').any(|seg| seg == "..") {
        return Err(AppError::Invalid(format!("非法路径: {relative}")));
    }
    Ok(root.join(rel))
}

/// 归一化为以 `/` 分隔的相对路径字符串。
pub fn to_relative_string(root: &Path, abs: &Path) -> String {
    abs.strip_prefix(root)
        .unwrap_or(abs)
        .to_string_lossy()
        .replace('\\', "/")
}

fn fallback_title(abs: &Path) -> String {
    abs.file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "未命名".into())
}

/// 读取并解析文章文件为 ArticleContent（标题缺失回退文件名，FR-016/Q2）。
pub fn read_article(root: &Path, relative: &str) -> AppResult<ArticleContent> {
    let abs = resolve_in_workspace(root, relative)?;
    let raw = std::fs::read_to_string(&abs)?;
    let parsed = frontmatter::parse(&raw);
    let fm = parsed.front_matter;
    let title = fm.title.clone().unwrap_or_else(|| fallback_title(&abs));
    Ok(ArticleContent {
        relative_path: relative.to_string(),
        title,
        tags: fm.tags,
        created: fm.created.unwrap_or_default(),
        updated: fm.updated.unwrap_or_default(),
        base_hash: frontmatter::content_hash(&parsed.body),
        body: parsed.body,
    })
}

/// 写入文章文件。返回写入后的 ArticleContent（含新 base_hash）。
pub fn write_article(
    root: &Path,
    relative: &str,
    title: &str,
    tags: &[String],
    body: &str,
    created: Option<String>,
) -> AppResult<ArticleContent> {
    let abs = resolve_in_workspace(root, relative)?;
    if let Some(parent) = abs.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let now = Utc::now().to_rfc3339();
    let created = created.unwrap_or_else(|| now.clone());
    let fm = FrontMatter {
        title: Some(title.to_string()),
        tags: tags.to_vec(),
        created: Some(created.clone()),
        updated: Some(now.clone()),
    };
    let content = frontmatter::serialize(&fm, body);
    std::fs::write(&abs, &content)?;
    Ok(ArticleContent {
        relative_path: relative.to_string(),
        title: title.to_string(),
        tags: tags.to_vec(),
        created,
        updated: now,
        base_hash: frontmatter::content_hash(body),
        body: body.to_string(),
    })
}

/// 读取磁盘文件当前正文哈希（用于保存前冲突检测，FR-019）。
pub fn current_hash(root: &Path, relative: &str) -> AppResult<Option<String>> {
    let abs = resolve_in_workspace(root, relative)?;
    match std::fs::read_to_string(&abs) {
        Ok(raw) => Ok(Some(frontmatter::content_hash(
            &frontmatter::parse(&raw).body,
        ))),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_path_traversal() {
        let root = Path::new("/tmp/ws");
        assert!(resolve_in_workspace(root, "../evil.md").is_err());
        assert!(resolve_in_workspace(root, "sub/../../evil.md").is_err());
        assert!(resolve_in_workspace(root, "ok/note.md").is_ok());
    }

    #[test]
    fn write_then_read_roundtrip() {
        let dir = std::env::temp_dir().join(format!("qp-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let saved = write_article(&dir, "a.md", "标题", &["x".into()], "正文body", None).unwrap();
        let read = read_article(&dir, "a.md").unwrap();
        assert_eq!(read.title, "标题");
        assert_eq!(read.tags, vec!["x"]);
        assert_eq!(read.body, "正文body");
        assert_eq!(read.base_hash, saved.base_hash);
        std::fs::remove_dir_all(&dir).ok();
    }
}
