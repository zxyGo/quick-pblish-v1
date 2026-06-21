use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// 文章 front matter 的落盘形态（YAML）。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FrontMatter {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated: Option<String>,
}

/// 解析结果：front matter（缺失/损坏时降级为 default）+ 正文。
pub struct ParsedArticle {
    pub front_matter: FrontMatter,
    pub body: String,
}

/// 解析 Markdown 文本，分离 YAML front matter 与正文。
/// 缺失或损坏的 front matter 降级为 default，绝不报错中断（FR-018）。
pub fn parse(content: &str) -> ParsedArticle {
    let normalized = content.strip_prefix('\u{feff}').unwrap_or(content);
    if let Some(rest) = normalized.strip_prefix("---") {
        // 第一行剩余必须是换行
        if let Some(after_open) = strip_line_break(rest) {
            if let Some((yaml, body)) = split_closing_fence(after_open) {
                let front_matter = serde_yaml::from_str::<FrontMatter>(yaml).unwrap_or_default();
                return ParsedArticle {
                    front_matter,
                    body: body.to_string(),
                };
            }
        }
    }
    ParsedArticle {
        front_matter: FrontMatter::default(),
        body: normalized.to_string(),
    }
}

/// 序列化为 `---\n<yaml>---\n<body>`。无任何字段时仅返回正文。
pub fn serialize(front_matter: &FrontMatter, body: &str) -> String {
    let is_empty = front_matter.title.is_none()
        && front_matter.tags.is_empty()
        && front_matter.created.is_none()
        && front_matter.updated.is_none();
    if is_empty {
        return body.to_string();
    }
    let yaml = serde_yaml::to_string(front_matter).unwrap_or_default();
    format!("---\n{yaml}---\n{body}")
}

/// 正文内容哈希（用于乐观冲突检测，FR-019）。
pub fn content_hash(body: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(body.as_bytes());
    hex::encode(hasher.finalize())
}

/// 摘要：取正文前 N 个字符（去除换行）。
pub fn excerpt(body: &str, max_chars: usize) -> String {
    let cleaned: String = body
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    cleaned.chars().take(max_chars).collect()
}

fn strip_line_break(s: &str) -> Option<&str> {
    if let Some(r) = s.strip_prefix("\r\n") {
        Some(r)
    } else {
        s.strip_prefix('\n')
    }
}

/// 在 after_open 中找到独占一行的 `---`，返回 (yaml, body)。
fn split_closing_fence(after_open: &str) -> Option<(&str, &str)> {
    let mut search_start = 0usize;
    let bytes = after_open.as_bytes();
    loop {
        let idx = after_open[search_start..].find("---")?;
        let abs = search_start + idx;
        let at_line_start = abs == 0 || bytes[abs - 1] == b'\n';
        let after = &after_open[abs + 3..];
        let at_line_end = after.is_empty() || after.starts_with('\n') || after.starts_with("\r\n");
        if at_line_start && at_line_end {
            let yaml = &after_open[..abs];
            let body = strip_line_break(after).unwrap_or(after);
            return Some((yaml, body));
        }
        search_start = abs + 3;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_roundtrip_preserves_body() {
        let fm = FrontMatter {
            title: Some("标题".into()),
            tags: vec!["a".into(), "b".into()],
            created: Some("2026-06-21T10:00:00".into()),
            updated: Some("2026-06-21T12:00:00".into()),
        };
        let body = "# Hello\n\n正文内容";
        let serialized = serialize(&fm, body);
        let parsed = parse(&serialized);
        assert_eq!(parsed.body, body);
        assert_eq!(parsed.front_matter.title.as_deref(), Some("标题"));
        assert_eq!(parsed.front_matter.tags, vec!["a", "b"]);
    }

    #[test]
    fn parse_without_front_matter_returns_full_body() {
        let content = "# 没有 front matter\n正文";
        let parsed = parse(content);
        assert!(parsed.front_matter.title.is_none());
        assert_eq!(parsed.body, content);
    }

    #[test]
    fn corrupt_front_matter_degrades_to_default() {
        // YAML 非法，但仍应分离出正文而不 panic（FR-018）
        let content = "---\ntitle: [unclosed\n---\n正文";
        let parsed = parse(content);
        assert!(parsed.front_matter.title.is_none());
        assert_eq!(parsed.body, "正文");
    }

    #[test]
    fn hash_changes_with_body() {
        assert_ne!(content_hash("a"), content_hash("b"));
        assert_eq!(content_hash("same"), content_hash("same"));
    }
}
