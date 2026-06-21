use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::AppResult;

/// 工作目录配置（存于 OS 应用配置目录，非用户内容目录）。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkspaceConfig {
    pub current: Option<String>,
    #[serde(default)]
    pub recent: Vec<String>,
}

const MAX_RECENT: usize = 10;

fn config_file(config_dir: &Path) -> PathBuf {
    config_dir.join("workspace.json")
}

pub fn load(config_dir: &Path) -> WorkspaceConfig {
    let path = config_file(config_dir);
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save(config_dir: &Path, config: &WorkspaceConfig) -> AppResult<()> {
    std::fs::create_dir_all(config_dir)?;
    let path = config_file(config_dir);
    std::fs::write(path, serde_json::to_string_pretty(config)?)?;
    Ok(())
}

/// 将某目录设为 current 并提升到 recent 列表首位。
pub fn set_current(config: &mut WorkspaceConfig, path: &str) {
    config.current = Some(path.to_string());
    config.recent.retain(|p| p != path);
    config.recent.insert(0, path.to_string());
    config.recent.truncate(MAX_RECENT);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_current_dedups_and_caps() {
        let mut c = WorkspaceConfig::default();
        for i in 0..15 {
            set_current(&mut c, &format!("/ws/{i}"));
        }
        assert_eq!(c.current.as_deref(), Some("/ws/14"));
        assert_eq!(c.recent.len(), MAX_RECENT);
        assert_eq!(c.recent[0], "/ws/14");
        // 重复加入不产生重复项
        set_current(&mut c, "/ws/14");
        assert_eq!(c.recent.iter().filter(|p| *p == "/ws/14").count(), 1);
    }
}
