use serde::Deserialize;
use tauri::State;

use crate::domain::{
    ArticleContent, ArticleSummary, ConflictStrategy, ListQuery, SaveArticleInput,
};
use crate::error::{AppError, AppResult};
use crate::index;
use crate::state::AppState;
use crate::storage::article_fs;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateArticleInput {
    pub relative_path: String,
    #[serde(default)]
    pub title: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateMetadataInput {
    pub relative_path: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub tags: Option<Vec<String>>,
}

/// 乐观冲突判定（FR-019）：磁盘当前哈希与打开时 base_hash 不一致即为冲突。
fn has_conflict(disk_hash: &Option<String>, base_hash: &str) -> bool {
    matches!(disk_hash, Some(d) if d != base_hash)
}

// ---- core 实现（接收 &AppState，便于单元测试，不依赖 Tauri 运行时）----

pub(crate) fn list_articles_core(
    state: &AppState,
    query: &ListQuery,
) -> AppResult<Vec<ArticleSummary>> {
    let root = state.current_root()?;
    let conn = state.db.lock().expect("db lock");
    index::query(&conn, &root, query)
}

pub(crate) fn read_article_core(
    state: &AppState,
    relative_path: &str,
) -> AppResult<ArticleContent> {
    let root = state.current_root()?;
    article_fs::read_article(&root, relative_path)
}

pub(crate) fn create_article_core(
    state: &AppState,
    input: &CreateArticleInput,
) -> AppResult<ArticleContent> {
    let root = state.current_root()?;
    let abs = article_fs::resolve_in_workspace(&root, &input.relative_path)?;
    if abs.exists() {
        return Err(AppError::Conflict(format!(
            "文件已存在: {}",
            input.relative_path
        )));
    }
    let title = input.title.clone().unwrap_or_else(|| {
        abs.file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "未命名".into())
    });
    let content = article_fs::write_article(&root, &input.relative_path, &title, &[], "", None)?;
    {
        let conn = state.db.lock().expect("db lock");
        index::upsert(&conn, &root, &abs)?;
    }
    Ok(content)
}

pub(crate) fn save_article_core(
    state: &AppState,
    input: &SaveArticleInput,
) -> AppResult<ArticleContent> {
    let root = state.current_root()?;

    let disk_hash = article_fs::current_hash(&root, &input.relative_path)?;
    let mut target_path = input.relative_path.clone();
    if has_conflict(&disk_hash, &input.base_hash) {
        match input.on_conflict {
            ConflictStrategy::Abort => {
                return Err(AppError::Conflict(
                    "文件已被外部修改，请选择处理方式".into(),
                ));
            }
            ConflictStrategy::Overwrite => {}
            ConflictStrategy::SaveAs => {
                target_path = input
                    .save_as_path
                    .clone()
                    .ok_or_else(|| AppError::Invalid("缺少 saveAsPath".into()))?;
            }
        }
    }

    let created = article_fs::read_article(&root, &target_path)
        .ok()
        .map(|a| a.created)
        .filter(|c| !c.is_empty());

    let content = article_fs::write_article(
        &root,
        &target_path,
        &input.title,
        &input.tags,
        &input.body,
        created,
    )?;
    {
        let abs = article_fs::resolve_in_workspace(&root, &target_path)?;
        let conn = state.db.lock().expect("db lock");
        index::upsert(&conn, &root, &abs)?;
    }
    Ok(content)
}

pub(crate) fn delete_article_core(state: &AppState, relative_path: &str) -> AppResult<()> {
    let root = state.current_root()?;
    let abs = article_fs::resolve_in_workspace(&root, relative_path)?;
    trash::delete(&abs)?;
    {
        let conn = state.db.lock().expect("db lock");
        index::remove(&conn, &root, relative_path)?;
    }
    Ok(())
}

pub(crate) fn update_metadata_core(
    state: &AppState,
    input: &UpdateMetadataInput,
) -> AppResult<ArticleSummary> {
    let root = state.current_root()?;
    let existing = article_fs::read_article(&root, &input.relative_path)?;
    let title = input.title.clone().unwrap_or(existing.title);
    let tags = input.tags.clone().unwrap_or(existing.tags);
    let created = Some(existing.created).filter(|c| !c.is_empty());
    let content = article_fs::write_article(
        &root,
        &input.relative_path,
        &title,
        &tags,
        &existing.body,
        created,
    )?;
    {
        let abs = article_fs::resolve_in_workspace(&root, &input.relative_path)?;
        let conn = state.db.lock().expect("db lock");
        index::upsert(&conn, &root, &abs)?;
    }
    Ok(ArticleSummary {
        relative_path: content.relative_path,
        title: content.title,
        tags: content.tags,
        created: content.created,
        updated: content.updated,
        excerpt: crate::storage::frontmatter::excerpt(&content.body, 120),
    })
}

// ---- Tauri command 薄封装 ----

#[tauri::command]
pub fn list_articles(state: State<AppState>, query: ListQuery) -> AppResult<Vec<ArticleSummary>> {
    list_articles_core(state.inner(), &query)
}

#[tauri::command]
pub fn read_article(state: State<AppState>, relative_path: String) -> AppResult<ArticleContent> {
    read_article_core(state.inner(), &relative_path)
}

#[tauri::command]
pub fn create_article(
    state: State<AppState>,
    input: CreateArticleInput,
) -> AppResult<ArticleContent> {
    create_article_core(state.inner(), &input)
}

#[tauri::command]
pub fn save_article(state: State<AppState>, input: SaveArticleInput) -> AppResult<ArticleContent> {
    save_article_core(state.inner(), &input)
}

#[tauri::command]
pub fn delete_article(state: State<AppState>, relative_path: String) -> AppResult<()> {
    delete_article_core(state.inner(), &relative_path)
}

#[tauri::command]
pub fn update_metadata(
    state: State<AppState>,
    input: UpdateMetadataInput,
) -> AppResult<ArticleSummary> {
    update_metadata_core(state.inner(), &input)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::test_support::TestEnv;

    fn save_input(path: &str, body: &str, base_hash: &str) -> SaveArticleInput {
        SaveArticleInput {
            relative_path: path.into(),
            title: "标题".into(),
            tags: vec!["t".into()],
            body: body.into(),
            base_hash: base_hash.into(),
            on_conflict: ConflictStrategy::Abort,
            save_as_path: None,
        }
    }

    #[test]
    fn conflict_only_when_disk_differs_from_base() {
        assert!(!has_conflict(&None, "abc"));
        assert!(!has_conflict(&Some("abc".into()), "abc"));
        assert!(has_conflict(&Some("xyz".into()), "abc"));
    }

    #[test]
    fn create_read_save_roundtrip() {
        let env = TestEnv::new();
        // create
        let created = create_article_core(
            &env.state,
            &CreateArticleInput {
                relative_path: "a.md".into(),
                title: Some("我的文章".into()),
            },
        )
        .unwrap();
        assert_eq!(created.title, "我的文章");

        // 重复创建 → Conflict（FR-020）
        let dup = create_article_core(
            &env.state,
            &CreateArticleInput {
                relative_path: "a.md".into(),
                title: None,
            },
        );
        assert!(matches!(dup, Err(AppError::Conflict(_))));

        // read 返回 baseHash
        let read = read_article_core(&env.state, "a.md").unwrap();

        // save 正常
        let saved =
            save_article_core(&env.state, &save_input("a.md", "正文v2", &read.base_hash)).unwrap();
        assert_eq!(saved.body, "正文v2");

        // 用过期 baseHash 保存 + Abort → Conflict（FR-019）
        let stale = save_article_core(&env.state, &save_input("a.md", "正文v3", &read.base_hash));
        assert!(matches!(stale, Err(AppError::Conflict(_))));

        // Overwrite 策略可强制保存
        let mut overwrite = save_input("a.md", "正文v3", &read.base_hash);
        overwrite.on_conflict = ConflictStrategy::Overwrite;
        assert!(save_article_core(&env.state, &overwrite).is_ok());
    }

    #[test]
    fn list_search_sort_and_update_metadata() {
        let env = TestEnv::new();
        for (p, t) in [("alpha.md", "Alpha"), ("beta.md", "Beta")] {
            create_article_core(
                &env.state,
                &CreateArticleInput {
                    relative_path: p.into(),
                    title: Some(t.into()),
                },
            )
            .unwrap();
        }
        // 列表
        let all = list_articles_core(&env.state, &ListQuery::default()).unwrap();
        assert_eq!(all.len(), 2);

        // 检索
        let q = ListQuery {
            keyword: Some("Alpha".into()),
            ..Default::default()
        };
        let found = list_articles_core(&env.state, &q).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].title, "Alpha");

        // 更新标签（FR-016）
        let updated = update_metadata_core(
            &env.state,
            &UpdateMetadataInput {
                relative_path: "alpha.md".into(),
                title: None,
                tags: Some(vec!["x".into(), "y".into()]),
            },
        )
        .unwrap();
        assert_eq!(updated.tags, vec!["x", "y"]);

        // 按标签检索得到该文
        let q2 = ListQuery {
            keyword: Some("x".into()),
            ..Default::default()
        };
        assert_eq!(list_articles_core(&env.state, &q2).unwrap().len(), 1);
    }

    #[test]
    fn delete_moves_to_trash_and_removes_from_index() {
        let env = TestEnv::new();
        create_article_core(
            &env.state,
            &CreateArticleInput {
                relative_path: "gone.md".into(),
                title: None,
            },
        )
        .unwrap();
        assert_eq!(
            list_articles_core(&env.state, &ListQuery::default())
                .unwrap()
                .len(),
            1
        );
        delete_article_core(&env.state, "gone.md").unwrap();
        assert!(!env.ws.join("gone.md").exists());
        assert_eq!(
            list_articles_core(&env.state, &ListQuery::default())
                .unwrap()
                .len(),
            0
        );
    }
}
