//! 002-multi-platform-publish：发布编排（平台无关）。
//! 平台细节封装在 `crate::adapters`；本模块负责会话、WebView 桥接、同步编排与历史。

pub mod history;
pub mod session;
pub mod sync;
pub mod webview;

use serde::{Deserialize, Serialize};

use crate::adapters::PlatformId;

/// 平台连接状态（FR-003 / FR-006）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlatformStatus {
    Disconnected,
    Connected,
    NeedReauth,
}

/// 单个同步任务状态（FR-014）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SyncStatus {
    Pending,
    Running,
    Success,
    Failed,
}

/// 平台连接（data-model.md）。运行时派生，不长期信任缓存。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlatformConnection {
    pub platform: PlatformId,
    pub status: PlatformStatus,
    pub account_label: Option<String>,
    pub last_checked_at: Option<String>,
}

/// 平台草稿引用（FR-007 成功产物）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DraftRef {
    pub platform: PlatformId,
    pub draft_id: Option<String>,
    pub url: Option<String>,
}

/// 同步任务（FR-007/013/015/016）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncJob {
    pub id: String,
    pub article_path: String,
    pub platform: PlatformId,
    pub status: SyncStatus,
    pub failure_reason: Option<String>,
    pub draft_ref: Option<DraftRef>,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
}

impl SyncJob {
    pub fn pending(article_path: &str, platform: PlatformId) -> Self {
        SyncJob {
            id: format!("{}:{}", platform.as_str(), uuid_like()),
            article_path: article_path.to_string(),
            platform,
            status: SyncStatus::Pending,
            failure_reason: None,
            draft_ref: None,
            started_at: None,
            finished_at: None,
        }
    }
}

/// 同步入参（contracts/publish.md）。`rendered_html` 由前端用 @md/core 渲染（与预览同源）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncRequest {
    pub article_path: String,
    pub rendered_html: String,
    pub title: String,
    pub platforms: Vec<PlatformId>,
}

/// 同步历史记录（FR-018，SQLite 派生缓存）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncRecord {
    pub id: i64,
    pub article_path: String,
    pub platform: PlatformId,
    pub status: SyncStatus,
    pub failure_reason: Option<String>,
    pub draft_url: Option<String>,
    pub synced_at: String,
}

/// 简易唯一 id（避免额外引入 uuid 依赖）：时间戳纳秒 + 计数。
fn uuid_like() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{ts:x}{n:x}")
}
