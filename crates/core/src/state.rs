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
        std::fs::create_dir_all(
            self.settings_path
                .parent()
                .unwrap_or(std::path::Path::new(".")),
        )?;
        let json =
            serde_json::to_vec_pretty(&*self.settings.read()).map_err(std::io::Error::other)?;
        std::fs::write(&self.settings_path, json)
    }

    pub fn persist_recents(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(
            self.recents_path
                .parent()
                .unwrap_or(std::path::Path::new(".")),
        )?;
        let json =
            serde_json::to_vec_pretty(&*self.recents.read()).map_err(std::io::Error::other)?;
        std::fs::write(&self.recents_path, json)
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
}
