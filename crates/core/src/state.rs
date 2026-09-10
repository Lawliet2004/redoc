use crate::recents::RecentStore;
use crate::recovery::RecoveryManager;
use crate::settings::AppSettings;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct OpenDocumentInfo {
    pub id: String,
    pub path: Option<String>,
    pub title: String,
    pub mode: String,
    pub dirty: bool,
    pub content_hash: String,
}

pub struct AppState {
    pub settings: RwLock<AppSettings>,
    pub recents: RwLock<RecentStore>,
    pub open_docs: RwLock<HashMap<String, OpenDocumentInfo>>,
    pub recovery: Arc<RecoveryManager>,
    pub recovery_pending: bool,
    settings_path: std::path::PathBuf,
    recents_path: std::path::PathBuf,
}

/// Write bytes to a temp file next to the target, fsync, then rename over it
/// so a crash mid-write never leaves a truncated settings/recents file.
pub fn write_bytes_atomic(path: &std::path::Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let parent = path.parent().unwrap_or(std::path::Path::new("."));
    std::fs::create_dir_all(parent)?;
    let temp_path = parent.join(format!(".tmp_{}", uuid::Uuid::now_v7()));
    struct TempGuard(std::path::PathBuf);
    impl Drop for TempGuard {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
    let guard = TempGuard(temp_path.clone());
    {
        let mut file = std::fs::File::create(&temp_path)?;
        file.write_all(bytes)?;
        file.sync_all()?;
    }
    match std::fs::rename(&temp_path, path) {
        Ok(()) => {
            std::mem::forget(guard);
            Ok(())
        }
        Err(err) if !path.exists() => Err(err),
        Err(_) => {
            let target_name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("state");
            let backup_path = parent.join(format!(".{}.bak-{}", target_name, uuid::Uuid::now_v7()));
            std::fs::rename(path, &backup_path)?;
            match std::fs::rename(&temp_path, path) {
                Ok(()) => {
                    std::mem::forget(guard);
                    let _ = std::fs::remove_file(backup_path);
                    Ok(())
                }
                Err(rename_err) => {
                    std::fs::rename(&backup_path, path)?;
                    Err(rename_err)
                }
            }
        }
    }
}

impl AppState {
    pub fn new(app_data_dir: std::path::PathBuf) -> Self {
        let settings_path = app_data_dir.join("settings.json");
        let recents_path = app_data_dir.join("recents.json");
        let recovery = Arc::new(RecoveryManager::new(app_data_dir));
        let recovery_pending = recovery.had_unclean_shutdown();
        let settings = std::fs::read_to_string(&settings_path)
            .ok()
            .and_then(|json| serde_json::from_str(&json).ok())
            .unwrap_or_default();
        let recents = std::fs::read_to_string(&recents_path)
            .ok()
            .and_then(|json| serde_json::from_str(&json).ok())
            .unwrap_or_default();
        Self {
            settings: RwLock::new(settings),
            recents: RwLock::new(recents),
            open_docs: RwLock::new(HashMap::new()),
            recovery,
            recovery_pending,
            settings_path,
            recents_path,
        }
    }

    pub fn persist_settings(&self) -> std::io::Result<()> {
        let json =
            serde_json::to_vec_pretty(&*self.settings.read()).map_err(std::io::Error::other)?;
        write_bytes_atomic(&self.settings_path, &json)
    }

    pub fn persist_recents(&self) -> std::io::Result<()> {
        let json =
            serde_json::to_vec_pretty(&*self.recents.read()).map_err(std::io::Error::other)?;
        write_bytes_atomic(&self.recents_path, &json)
    }

    pub fn log_dir(&self) -> std::path::PathBuf {
        self.settings_path
            .parent()
            .unwrap_or(std::path::Path::new("."))
            .join("logs")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn persists_settings_and_recents() {
        let path = std::env::temp_dir().join(format!("redoc-state-{}", uuid::Uuid::now_v7()));
        let state = AppState::new(path.clone());
        state.settings.write().theme = "dark".to_string();
        state.recents.write().add(
            "example.redoc".to_string(),
            "Example".to_string(),
            "doc".to_string(),
        );
        state.persist_settings().expect("persist settings");
        state.persist_recents().expect("persist recents");

        let loaded = AppState::new(path.clone());
        assert_eq!(loaded.settings.read().theme, "dark");
        assert_eq!(loaded.recents.read().entries[0].title, "Example");
        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn write_bytes_atomic_replaces_and_cleans_temp_files() {
        let dir = std::env::temp_dir().join(format!("redoc-atomic-{}", uuid::Uuid::now_v7()));
        std::fs::create_dir_all(&dir).expect("create dir");
        let target = dir.join("state.json");
        write_bytes_atomic(&target, b"first").expect("initial write");
        write_bytes_atomic(&target, b"second").expect("replacement write");
        assert_eq!(std::fs::read(&target).expect("read target"), b"second");
        let leftovers = std::fs::read_dir(&dir)
            .expect("read dir")
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().starts_with(".tmp_"))
            .count();
        assert_eq!(leftovers, 0, "atomic state write left temp files behind");
        let _ = std::fs::remove_dir_all(dir);
    }
}
