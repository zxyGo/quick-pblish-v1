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

/// 文件结构变化后重建该工作目录缓存（简单稳健，规模可接受）。
fn refresh_index(state: &AppState, root: &Path) {
    if let Ok(conn) = state.db.lock() {
        let _ = crate::index::rebuild(&conn, root);
    }
}

// ---- core 实现 ----

pub(crate) fn get_file_tree_core(state: &AppState) -> AppResult<FileNode> {
    let root = state.current_root()?;
    build_node(&root, &root)
}

pub(crate) fn create_folder_core(
    state: &AppState,
    parent_relative_path: &str,
    name: &str,
) -> AppResult<FileNode> {
    let root = state.current_root()?;
    let parent = article_fs::resolve_in_workspace(&root, parent_relative_path)?;
    let target = parent.join(name);
    if target.exists() {
        return Err(AppError::Conflict(format!("已存在: {name}")));
    }
    std::fs::create_dir_all(&target)?;
    build_node(&root, &target)
}

pub(crate) fn rename_path_core(
    state: &AppState,
    relative_path: &str,
    new_name: &str,
) -> AppResult<FileNode> {
    let root = state.current_root()?;
    let abs = article_fs::resolve_in_workspace(&root, relative_path)?;
    let parent = abs
        .parent()
        .ok_or_else(|| AppError::Invalid("无父目录".into()))?;
    let target = parent.join(new_name);
    if target.exists() {
        return Err(AppError::Conflict(format!("目标已存在: {new_name}")));
    }
    std::fs::rename(&abs, &target)?;
    refresh_index(state, &root);
    build_node(&root, &target)
}

pub(crate) fn move_path_core(
    state: &AppState,
    relative_path: &str,
    target_dir_relative_path: &str,
) -> AppResult<FileNode> {
    let root = state.current_root()?;
    let abs = article_fs::resolve_in_workspace(&root, relative_path)?;
    let target_dir = article_fs::resolve_in_workspace(&root, target_dir_relative_path)?;
    let file_name = abs
        .file_name()
        .ok_or_else(|| AppError::Invalid("无文件名".into()))?;
    let target = target_dir.join(file_name);
    if target.exists() {
        return Err(AppError::Conflict("目标目录存在同名项".into()));
    }
    std::fs::create_dir_all(&target_dir)?;
    std::fs::rename(&abs, &target)?;
    refresh_index(state, &root);
    build_node(&root, &target)
}

pub(crate) fn delete_path_core(state: &AppState, relative_path: &str) -> AppResult<()> {
    let root = state.current_root()?;
    let abs = article_fs::resolve_in_workspace(&root, relative_path)?;
    trash::delete(&abs)?;
    refresh_index(state, &root);
    Ok(())
}

// ---- Tauri command 薄封装 ----

#[tauri::command]
pub fn get_file_tree(state: State<AppState>) -> AppResult<FileNode> {
    get_file_tree_core(state.inner())
}

#[tauri::command]
pub fn create_folder(
    state: State<AppState>,
    parent_relative_path: String,
    name: String,
) -> AppResult<FileNode> {
    create_folder_core(state.inner(), &parent_relative_path, &name)
}

#[tauri::command]
pub fn rename_path(
    state: State<AppState>,
    relative_path: String,
    new_name: String,
) -> AppResult<FileNode> {
    rename_path_core(state.inner(), &relative_path, &new_name)
}

#[tauri::command]
pub fn move_path(
    state: State<AppState>,
    relative_path: String,
    target_dir_relative_path: String,
) -> AppResult<FileNode> {
    move_path_core(state.inner(), &relative_path, &target_dir_relative_path)
}

#[tauri::command]
pub fn delete_path(state: State<AppState>, relative_path: String) -> AppResult<()> {
    delete_path_core(state.inner(), &relative_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::test_support::TestEnv;

    #[test]
    fn build_node_mirrors_disk_and_marks_articles() {
        let root = std::env::temp_dir().join(format!(
            "qp-tree-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(root.join("sub")).unwrap();
        std::fs::write(root.join("a.md"), "# a").unwrap();
        std::fs::write(root.join("img.png"), "x").unwrap();
        std::fs::write(root.join("sub").join("b.md"), "# b").unwrap();

        let tree = build_node(&root, &root).unwrap();
        assert_eq!(tree.kind, NodeKind::Directory);
        let a = tree.children.iter().find(|n| n.name == "a.md").unwrap();
        assert!(a.is_article);
        let img = tree.children.iter().find(|n| n.name == "img.png").unwrap();
        assert!(!img.is_article);
        assert_eq!(tree.children.first().unwrap().name, "sub");

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn folder_ops_sync_to_disk_with_conflict_guard() {
        let env = TestEnv::new();
        // 新建文件夹
        create_folder_core(&env.state, "", "docs").unwrap();
        assert!(env.ws.join("docs").is_dir());
        // 重名 → Conflict（FR-013/FR-020）
        assert!(matches!(
            create_folder_core(&env.state, "", "docs"),
            Err(AppError::Conflict(_))
        ));

        // 创建文件并移动到 docs
        std::fs::write(env.ws.join("note.md"), "# n").unwrap();
        move_path_core(&env.state, "note.md", "docs").unwrap();
        assert!(env.ws.join("docs/note.md").exists());
        assert!(!env.ws.join("note.md").exists());

        // 重命名
        rename_path_core(&env.state, "docs/note.md", "renamed.md").unwrap();
        assert!(env.ws.join("docs/renamed.md").exists());

        // 删除入回收站
        delete_path_core(&env.state, "docs/renamed.md").unwrap();
        assert!(!env.ws.join("docs/renamed.md").exists());

        // 文件树反映结构
        let tree = get_file_tree_core(&env.state).unwrap();
        assert!(tree.children.iter().any(|n| n.name == "docs"));
    }
}
