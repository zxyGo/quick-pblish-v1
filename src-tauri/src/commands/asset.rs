use std::path::PathBuf;

use tauri::State;

use crate::domain::{ImportedAsset, IndexStatus};
use crate::error::{AppError, AppResult};
use crate::index;
use crate::state::AppState;

// ---- core 实现 ----

/// 将外部图片复制进工作目录 `assets/`，返回相对路径供正文引用（FR-014a/Q4）。
pub(crate) fn import_asset_core(state: &AppState, source_path: &str) -> AppResult<ImportedAsset> {
    let root = state.current_root()?;
    let source = PathBuf::from(source_path);
    let file_name = source
        .file_name()
        .ok_or_else(|| AppError::Invalid("无效的源文件".into()))?
        .to_string_lossy()
        .to_string();

    let assets_dir = root.join("assets");
    std::fs::create_dir_all(&assets_dir)?;

    // 去重命名，避免覆盖已有素材
    let mut target = assets_dir.join(&file_name);
    let stem = source
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "asset".into());
    let ext = source
        .extension()
        .map(|s| format!(".{}", s.to_string_lossy()))
        .unwrap_or_default();
    let mut counter = 1;
    while target.exists() {
        target = assets_dir.join(format!("{stem}-{counter}{ext}"));
        counter += 1;
    }

    std::fs::copy(&source, &target)?;
    let final_name = target
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or(file_name);

    Ok(ImportedAsset {
        relative_path: format!("assets/{final_name}"),
        file_name: final_name,
    })
}

pub(crate) fn rebuild_index_core(state: &AppState) -> AppResult<IndexStatus> {
    let root = state.current_root()?;
    let conn = state.db.lock().expect("db lock");
    index::rebuild(&conn, &root)?;
    index::status(&conn, &root)
}

pub(crate) fn index_status_core(state: &AppState) -> AppResult<IndexStatus> {
    let root = state.current_root()?;
    let conn = state.db.lock().expect("db lock");
    index::status(&conn, &root)
}

// ---- Tauri command 薄封装 ----

#[tauri::command]
pub fn import_asset(state: State<AppState>, source_path: String) -> AppResult<ImportedAsset> {
    import_asset_core(state.inner(), &source_path)
}

#[tauri::command]
pub fn rebuild_index(state: State<AppState>) -> AppResult<IndexStatus> {
    rebuild_index_core(state.inner())
}

#[tauri::command]
pub fn get_index_status(state: State<AppState>) -> AppResult<IndexStatus> {
    index_status_core(state.inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::test_support::TestEnv;

    #[test]
    fn import_asset_copies_and_dedups() {
        let env = TestEnv::new();
        let src = env.base.join("pic.png");
        std::fs::write(&src, "imgdata").unwrap();

        let a1 = import_asset_core(&env.state, &src.to_string_lossy()).unwrap();
        assert_eq!(a1.relative_path, "assets/pic.png");
        assert!(env.ws.join("assets/pic.png").exists());

        // 再次导入同名 → 去重命名，不覆盖
        let a2 = import_asset_core(&env.state, &src.to_string_lossy()).unwrap();
        assert_eq!(a2.relative_path, "assets/pic-1.png");
    }

    #[test]
    fn rebuild_index_counts_articles() {
        let env = TestEnv::new();
        std::fs::write(env.ws.join("x.md"), "---\ntitle: X\n---\nbody").unwrap();
        let status = rebuild_index_core(&env.state).unwrap();
        assert_eq!(status.total, 1);
        assert!(!status.rebuilding);
    }
}
