//! 同步历史（FR-018）。SQLite 派生缓存表 `sync_record`，可清空/重建，非真相来源（章程原则 I）。

use rusqlite::Connection;

use crate::adapters::PlatformId;
use crate::error::AppResult;
use crate::publish::{SyncRecord, SyncStatus};

/// 建表（随 index 一同初始化）。
pub fn init(conn: &Connection) -> AppResult<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS sync_record (
            id             INTEGER PRIMARY KEY AUTOINCREMENT,
            article_path   TEXT NOT NULL,
            platform       TEXT NOT NULL,
            status         TEXT NOT NULL,
            failure_reason TEXT,
            draft_url      TEXT,
            synced_at      TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_sync_record_article
            ON sync_record (article_path, synced_at DESC);",
    )?;
    Ok(())
}

fn status_str(s: SyncStatus) -> &'static str {
    match s {
        SyncStatus::Pending => "Pending",
        SyncStatus::Running => "Running",
        SyncStatus::Success => "Success",
        SyncStatus::Failed => "Failed",
    }
}

fn status_from(s: &str) -> SyncStatus {
    match s {
        "Success" => SyncStatus::Success,
        "Failed" => SyncStatus::Failed,
        "Running" => SyncStatus::Running,
        _ => SyncStatus::Pending,
    }
}

/// 写入一条历史。写历史失败不应影响同步主流程成败判定（调用方据此处理）。
pub fn insert(
    conn: &Connection,
    article_path: &str,
    platform: PlatformId,
    status: SyncStatus,
    failure_reason: Option<&str>,
    draft_url: Option<&str>,
    synced_at: &str,
) -> AppResult<()> {
    conn.execute(
        "INSERT INTO sync_record
            (article_path, platform, status, failure_reason, draft_url, synced_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![
            article_path,
            platform.as_str(),
            status_str(status),
            failure_reason,
            draft_url,
            synced_at
        ],
    )?;
    Ok(())
}

/// 按文章查询历史，按时间倒序（US4 / FR-018）。
pub fn list_by_article(conn: &Connection, article_path: &str) -> AppResult<Vec<SyncRecord>> {
    let mut stmt = conn.prepare(
        "SELECT id, article_path, platform, status, failure_reason, draft_url, synced_at
         FROM sync_record WHERE article_path = ?1 ORDER BY synced_at DESC, id DESC",
    )?;
    let rows = stmt.query_map([article_path], |row| {
        let platform: String = row.get(2)?;
        let status: String = row.get(3)?;
        Ok(SyncRecord {
            id: row.get(0)?,
            article_path: row.get(1)?,
            platform: platform.parse().unwrap_or(PlatformId::Weixin),
            status: status_from(&status),
            failure_reason: row.get(4)?,
            draft_url: row.get(5)?,
            synced_at: row.get(6)?,
        })
    })?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_and_list_desc() {
        let conn = Connection::open_in_memory().unwrap();
        init(&conn).unwrap();
        insert(&conn, "a.md", PlatformId::Weixin, SyncStatus::Success, None, Some("u1"), "2026-06-21T10:00:00Z").unwrap();
        insert(&conn, "a.md", PlatformId::Zhihu, SyncStatus::Failed, Some("auth"), None, "2026-06-21T11:00:00Z").unwrap();
        insert(&conn, "b.md", PlatformId::Juejin, SyncStatus::Success, None, None, "2026-06-21T12:00:00Z").unwrap();

        let recs = list_by_article(&conn, "a.md").unwrap();
        assert_eq!(recs.len(), 2);
        // 倒序：11:00 在前
        assert_eq!(recs[0].platform, PlatformId::Zhihu);
        assert_eq!(recs[0].status, SyncStatus::Failed);
        assert_eq!(recs[1].platform, PlatformId::Weixin);
    }
}
