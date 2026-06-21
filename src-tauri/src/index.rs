use std::path::{Path, PathBuf};

use rusqlite::Connection;
use walkdir::WalkDir;

use crate::domain::{ArticleSummary, IndexStatus, ListQuery};
use crate::error::{AppError, AppResult};
use crate::storage::frontmatter;

/// 打开/创建派生缓存数据库并建表。
/// 该缓存可随时从 Markdown 文件重建，非真相来源（FR-008a）。
pub fn open(db_path: &Path) -> AppResult<Connection> {
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let conn = Connection::open(db_path)?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS articles (
            workspace     TEXT NOT NULL,
            relative_path TEXT NOT NULL,
            title         TEXT NOT NULL,
            tags          TEXT NOT NULL,
            created       TEXT NOT NULL,
            updated       TEXT NOT NULL,
            excerpt       TEXT NOT NULL,
            body          TEXT NOT NULL,
            size          INTEGER NOT NULL,
            mtime         INTEGER NOT NULL,
            content_hash  TEXT NOT NULL,
            PRIMARY KEY (workspace, relative_path)
        );",
    )?;
    Ok(conn)
}

fn workspace_key(root: &Path) -> String {
    root.to_string_lossy().replace('\\', "/")
}

fn is_markdown(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|e| e.to_str()),
        Some("md") | Some("markdown")
    )
}

/// 从工作目录全量重建该目录的缓存条目（FR-008a / rebuild_index）。
pub fn rebuild(conn: &Connection, root: &Path) -> AppResult<u64> {
    let ws = workspace_key(root);
    conn.execute("DELETE FROM articles WHERE workspace = ?1", [&ws])?;
    let mut count = 0u64;
    for entry in WalkDir::new(root).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if entry.file_type().is_file() && is_markdown(path) {
            upsert(conn, root, path)?;
            count += 1;
        }
    }
    Ok(count)
}

/// 将单个文章文件插入/更新缓存。
pub fn upsert(conn: &Connection, root: &Path, abs: &Path) -> AppResult<()> {
    let raw = std::fs::read_to_string(abs)?;
    let parsed = frontmatter::parse(&raw);
    let fm = parsed.front_matter;
    let relative = crate::storage::article_fs::to_relative_string(root, abs);
    let title = fm.title.clone().unwrap_or_else(|| {
        abs.file_stem()
            .map(|s| s.to_string_lossy().into())
            .unwrap_or_default()
    });
    let meta = std::fs::metadata(abs)?;
    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let ws = workspace_key(root);
    conn.execute(
        "INSERT INTO articles
            (workspace, relative_path, title, tags, created, updated, excerpt, body, size, mtime, content_hash)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)
         ON CONFLICT(workspace, relative_path) DO UPDATE SET
            title=excluded.title, tags=excluded.tags, created=excluded.created,
            updated=excluded.updated, excerpt=excluded.excerpt, body=excluded.body,
            size=excluded.size, mtime=excluded.mtime, content_hash=excluded.content_hash",
        rusqlite::params![
            ws,
            relative,
            title,
            serde_json::to_string(&fm.tags)?,
            fm.created.unwrap_or_default(),
            fm.updated.unwrap_or_default(),
            frontmatter::excerpt(&parsed.body, 120),
            parsed.body,
            meta.len() as i64,
            mtime,
            frontmatter::content_hash(&parsed.body),
        ],
    )?;
    Ok(())
}

pub fn remove(conn: &Connection, root: &Path, relative: &str) -> AppResult<()> {
    let ws = workspace_key(root);
    conn.execute(
        "DELETE FROM articles WHERE workspace = ?1 AND relative_path = ?2",
        rusqlite::params![ws, relative],
    )?;
    Ok(())
}

/// 列表/检索查询（FR-015/FR-017）。检索覆盖标题/标签/正文。
pub fn query(conn: &Connection, root: &Path, q: &ListQuery) -> AppResult<Vec<ArticleSummary>> {
    let ws = workspace_key(root);
    let order_col = match q.sort_by.as_deref() {
        Some("created") => "created",
        Some("title") => "title",
        _ => "updated",
    };
    let order_dir = match q.order.as_deref() {
        Some("asc") => "ASC",
        _ => "DESC",
    };
    let mut sql = String::from(
        "SELECT relative_path, title, tags, created, updated, excerpt
         FROM articles WHERE workspace = ?1",
    );
    let keyword = q.keyword.clone().unwrap_or_default();
    let like = format!("%{keyword}%");
    if !keyword.is_empty() {
        sql.push_str(" AND (title LIKE ?2 OR tags LIKE ?2 OR body LIKE ?2)");
    }
    sql.push_str(&format!(" ORDER BY {order_col} {order_dir}"));

    let mut stmt = conn.prepare(&sql)?;
    let map_row = |row: &rusqlite::Row| -> rusqlite::Result<ArticleSummary> {
        let tags_json: String = row.get(2)?;
        Ok(ArticleSummary {
            relative_path: row.get(0)?,
            title: row.get(1)?,
            tags: serde_json::from_str(&tags_json).unwrap_or_default(),
            created: row.get(3)?,
            updated: row.get(4)?,
            excerpt: row.get(5)?,
        })
    };
    let rows = if keyword.is_empty() {
        stmt.query_map(rusqlite::params![ws], map_row)?
            .collect::<Result<Vec<_>, _>>()?
    } else {
        stmt.query_map(rusqlite::params![ws, like], map_row)?
            .collect::<Result<Vec<_>, _>>()?
    };
    Ok(rows)
}

pub fn status(conn: &Connection, root: &Path) -> AppResult<IndexStatus> {
    let ws = workspace_key(root);
    let total: i64 = conn.query_row(
        "SELECT COUNT(*) FROM articles WHERE workspace = ?1",
        [&ws],
        |r| r.get(0),
    )?;
    Ok(IndexStatus {
        total: total as u64,
        rebuilding: false,
    })
}

/// 默认数据库文件路径（OS 应用数据目录下）。
pub fn default_db_path(data_dir: &Path) -> PathBuf {
    data_dir.join("index.sqlite")
}

#[allow(dead_code)]
fn _assert_send(e: AppError) -> AppError {
    e
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_ws() -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("qp-idx-{}-{}", std::process::id(), rand_suffix()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn rand_suffix() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64
    }

    #[test]
    fn rebuild_and_query() {
        let ws = temp_ws();
        std::fs::write(
            ws.join("a.md"),
            "---\ntitle: Alpha\ntags: [rust]\n---\nhello world",
        )
        .unwrap();
        std::fs::write(ws.join("b.md"), "no front matter here").unwrap();

        let conn = open(&ws.join("index.sqlite")).unwrap();
        let n = rebuild(&conn, &ws).unwrap();
        assert_eq!(n, 2);

        let all = query(&conn, &ws, &ListQuery::default()).unwrap();
        assert_eq!(all.len(), 2);

        let mut q = ListQuery {
            keyword: Some("Alpha".into()),
            ..Default::default()
        };
        let found = query(&conn, &ws, &q).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].title, "Alpha");

        // 正文检索
        q.keyword = Some("hello".into());
        assert_eq!(query(&conn, &ws, &q).unwrap().len(), 1);

        std::fs::remove_dir_all(&ws).ok();
    }

    /// T044 性能抽查（SC-003）：1000 篇文章下列表查询首屏 ≤ 2s。
    /// 这里度量的是列表数据来源——派生缓存的 `query` 耗时（前端渲染另计）。
    #[test]
    fn perf_list_1000_articles_query_under_2s() {
        let ws = temp_ws();
        for i in 0..1000 {
            std::fs::write(
                ws.join(format!("article-{i:04}.md")),
                format!(
                    "---\ntitle: Article {i}\ntags: [t{}, bench]\ncreated: 2026-06-{:02}\n---\n正文内容 number {i} lorem ipsum dolor sit amet.",
                    i % 10,
                    (i % 28) + 1
                ),
            )
            .unwrap();
        }
        let conn = open(&ws.join("index.sqlite")).unwrap();
        let n = rebuild(&conn, &ws).unwrap();
        assert_eq!(n, 1000);

        // 全量列表首屏
        let start = std::time::Instant::now();
        let all = query(&conn, &ws, &ListQuery::default()).unwrap();
        let elapsed = start.elapsed();
        assert_eq!(all.len(), 1000);
        assert!(
            elapsed.as_millis() <= 2000,
            "全量列表查询耗时 {elapsed:?} 超过 2s 阈值"
        );

        // 关键字检索路径同样应在阈值内
        let q = ListQuery {
            keyword: Some("number 5".into()),
            ..Default::default()
        };
        let start = std::time::Instant::now();
        let found = query(&conn, &ws, &q).unwrap();
        let elapsed = start.elapsed();
        assert!(!found.is_empty());
        assert!(
            elapsed.as_millis() <= 2000,
            "检索查询耗时 {elapsed:?} 超过 2s 阈值"
        );

        std::fs::remove_dir_all(&ws).ok();
    }

    /// T046 派生缓存可重建端到端验证（FR-008a）：
    /// 删除 SQLite 缓存文件后，从 Markdown 重新打开并重建，数据完整无丢失。
    #[test]
    fn rebuild_after_cache_deleted_recovers_all_data() {
        let ws = temp_ws();
        for i in 0..5 {
            std::fs::write(
                ws.join(format!("note-{i}.md")),
                format!("---\ntitle: Note {i}\ntags: [keep]\n---\n可恢复正文 {i}"),
            )
            .unwrap();
        }
        let db_path = ws.join("index.sqlite");

        // 首次构建
        let conn = open(&db_path).unwrap();
        assert_eq!(rebuild(&conn, &ws).unwrap(), 5);
        let before = query(&conn, &ws, &ListQuery::default()).unwrap();
        assert_eq!(before.len(), 5);
        drop(conn);

        // 模拟缓存丢失：删除 SQLite 文件
        std::fs::remove_file(&db_path).unwrap();
        assert!(!db_path.exists());

        // 重启：重新打开（建空表）并从 Markdown 重建
        let conn2 = open(&db_path).unwrap();
        assert_eq!(rebuild(&conn2, &ws).unwrap(), 5);
        let after = query(&conn2, &ws, &ListQuery::default()).unwrap();
        assert_eq!(after.len(), 5, "重建后条目数应与原来一致");

        // 内容完整：标题集合一致
        let mut titles: Vec<_> = after.iter().map(|a| a.title.clone()).collect();
        titles.sort();
        assert_eq!(
            titles,
            vec!["Note 0", "Note 1", "Note 2", "Note 3", "Note 4"]
        );

        std::fs::remove_dir_all(&ws).ok();
    }
}
