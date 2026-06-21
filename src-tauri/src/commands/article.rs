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

#[tauri::command]
pub fn list_articles(state: State<AppState>, query: ListQuery) -> AppResult<Vec<ArticleSummary>> {
    let root = state.current_root()?;
    let conn = state.db.lock().expect("db lock");
    index::query(&conn, &root, &query)
}

#[tauri::command]
pub fn read_article(state: State<AppState>, relative_path: String) -> AppResult<ArticleContent> {
    let root = state.current_root()?;
    article_fs::read_article(&root, &relative_path)
}

#[tauri::command]
pub fn create_article(
    state: State<AppState>,
    input: CreateArticleInput,
) -> AppResult<ArticleContent> {
    let root = state.current_root()?;
    let abs = article_fs::resolve_in_workspace(&root, &input.relative_path)?;
    if abs.exists() {
        return Err(AppError::Conflict(format!(
            "文件已存在: {}",
            input.relative_path
        )));
    }
    let title = input.title.unwrap_or_else(|| {
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

#[tauri::command]
pub fn save_article(state: State<AppState>, input: SaveArticleInput) -> AppResult<ArticleContent> {
    let root = state.current_root()?;

    // 乐观冲突检测（FR-019）
    let disk_hash = article_fs::current_hash(&root, &input.relative_path)?;
    let mut target_path = input.relative_path.clone();
    if let Some(disk) = &disk_hash {
        if *disk != input.base_hash {
            match input.on_conflict {
                ConflictStrategy::Abort => {
                    return Err(AppError::Conflict(
                        "文件已被外部修改，请选择处理方式".into(),
                    ));
                }
                ConflictStrategy::Overwrite => { /* 继续写入当前路径 */ }
                ConflictStrategy::SaveAs => {
                    target_path = input
                        .save_as_path
                        .clone()
                        .ok_or_else(|| AppError::Invalid("缺少 saveAsPath".into()))?;
                }
            }
        }
    }

    // 保留原 created（若存在）
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

#[tauri::command]
pub fn delete_article(state: State<AppState>, relative_path: String) -> AppResult<()> {
    let root = state.current_root()?;
    let abs = article_fs::resolve_in_workspace(&root, &relative_path)?;
    trash::delete(&abs)?;
    {
        let conn = state.db.lock().expect("db lock");
        index::remove(&conn, &root, &relative_path)?;
    }
    Ok(())
}

#[tauri::command]
pub fn update_metadata(
    state: State<AppState>,
    input: UpdateMetadataInput,
) -> AppResult<ArticleSummary> {
    let root = state.current_root()?;
    let existing = article_fs::read_article(&root, &input.relative_path)?;
    let title = input.title.unwrap_or(existing.title);
    let tags = input.tags.unwrap_or(existing.tags);
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
