use std::path::Path;

use tauri::State;

use crate::domain::{FileNode, NodeKind};
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use crate::storage::article_fs;

fn is_markdown(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|e| e.to_str()),
        Some("md") | Some("markdown")
    )
}

fn build_node(root: &Path, abs: &Path) -> AppResult<FileNode> {
    let is_dir = abs.is_dir();
    let name = abs
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    let mut children = Vec::new();
    if is_dir {
        let mut entries: Vec<_> = std::fs::read_dir(abs)?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .collect();
        // 目录在前，按名称排序
        entries.sort_by(|a, b| {
            let ad = a.is_dir();
            let bd = b.is_dir();
            bd.cmp(&ad).then_with(|| a.file_name().cmp(&b.file_name()))
        });
        for child in entries {
            children.push(build_node(root, &child)?);
        }
    }
    Ok(FileNode {
        relative_path: article_fs::to_relative_string(root, abs),
        name,
        kind: if is_dir {
            NodeKind::Directory
        } else {
            NodeKind::File
        },
        is_article: !is_dir && is_markdown(abs),
        children,
    })
}

#[tauri::command]
pub fn get_file_tree(state: State<AppState>) -> AppResult<FileNode> {
    let root = state.current_root()?;
    build_node(&root, &root)
}

#[tauri::command]
pub fn create_folder(
    state: State<AppState>,
    parent_relative_path: String,
    name: String,
) -> AppResult<FileNode> {
    let root = state.current_root()?;
    let parent = article_fs::resolve_in_workspace(&root, &parent_relative_path)?;
    let target = parent.join(&name);
    if target.exists() {
        return Err(AppError::Conflict(format!("已存在: {name}")));
    }
    std::fs::create_dir_all(&target)?;
    build_node(&root, &target)
}

#[tauri::command]
pub fn rename_path(
    state: State<AppState>,
    relative_path: String,
    new_name: String,
) -> AppResult<FileNode> {
    let root = state.current_root()?;
    let abs = article_fs::resolve_in_workspace(&root, &relative_path)?;
    let parent = abs
        .parent()
        .ok_or_else(|| AppError::Invalid("无父目录".into()))?;
    let target = parent.join(&new_name);
    if target.exists() {
        return Err(AppError::Conflict(format!("目标已存在: {new_name}")));
    }
    std::fs::rename(&abs, &target)?;
    refresh_index(&state, &root);
    build_node(&root, &target)
}

#[tauri::command]
pub fn move_path(
    state: State<AppState>,
    relative_path: String,
    target_dir_relative_path: String,
) -> AppResult<FileNode> {
    let root = state.current_root()?;
    let abs = article_fs::resolve_in_workspace(&root, &relative_path)?;
    let target_dir = article_fs::resolve_in_workspace(&root, &target_dir_relative_path)?;
    let file_name = abs
        .file_name()
        .ok_or_else(|| AppError::Invalid("无文件名".into()))?;
    let target = target_dir.join(file_name);
    if target.exists() {
        return Err(AppError::Conflict("目标目录存在同名项".into()));
    }
    std::fs::create_dir_all(&target_dir)?;
    std::fs::rename(&abs, &target)?;
    refresh_index(&state, &root);
    build_node(&root, &target)
}

#[tauri::command]
pub fn delete_path(state: State<AppState>, relative_path: String) -> AppResult<()> {
    let root = state.current_root()?;
    let abs = article_fs::resolve_in_workspace(&root, &relative_path)?;
    trash::delete(&abs)?;
    refresh_index(&state, &root);
    Ok(())
}

/// 文件结构变化后重建该工作目录缓存（简单稳健，规模可接受）。
fn refresh_index(state: &State<AppState>, root: &Path) {
    if let Ok(conn) = state.db.lock() {
        let _ = crate::index::rebuild(&conn, root);
    }
}
