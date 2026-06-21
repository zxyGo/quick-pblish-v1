use serde::{Deserialize, Serialize};

/// 工作目录（应用级配置，非内容的一部分）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Workspace {
    pub path: String,
    pub name: String,
    pub last_opened: String,
}

/// 文章摘要（用于列表展示，来自派生缓存）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArticleSummary {
    pub relative_path: String,
    pub title: String,
    pub tags: Vec<String>,
    pub created: String,
    pub updated: String,
    pub excerpt: String,
}

/// 文章完整内容（编辑器读取）。`base_hash` 用于保存时的乐观冲突检测。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArticleContent {
    pub relative_path: String,
    pub title: String,
    pub tags: Vec<String>,
    pub created: String,
    pub updated: String,
    pub body: String,
    pub base_hash: String,
}

/// 文件树节点（映射磁盘结构）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileNode {
    pub relative_path: String,
    pub name: String,
    pub kind: NodeKind,
    pub is_article: bool,
    pub children: Vec<FileNode>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum NodeKind {
    File,
    Directory,
}

/// 导入素材的结果。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportedAsset {
    pub relative_path: String,
    pub file_name: String,
}

/// 派生缓存索引状态。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexStatus {
    pub total: u64,
    pub rebuilding: bool,
}

/// 保存文章的入参，冲突处理策略匹配 contracts/article.md。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveArticleInput {
    pub relative_path: String,
    pub title: String,
    pub tags: Vec<String>,
    pub body: String,
    pub base_hash: String,
    #[serde(default)]
    pub on_conflict: ConflictStrategy,
    #[serde(default)]
    pub save_as_path: Option<String>,
}

#[derive(Debug, Clone, Copy, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ConflictStrategy {
    #[default]
    Abort,
    Overwrite,
    SaveAs,
}

/// 列表查询参数。
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ListQuery {
    #[serde(default)]
    pub keyword: Option<String>,
    #[serde(default)]
    pub sort_by: Option<String>,
    #[serde(default)]
    pub order: Option<String>,
}
