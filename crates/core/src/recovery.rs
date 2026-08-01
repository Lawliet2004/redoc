use serde::{Deserialize, Serialize};
use specta::Type;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RecoveredDoc {
    pub id: String,
    pub title: String,
    pub mode: String,
    pub path: Option<String>,
    pub snapshot_path: String,
    pub timestamp: u64,
}

pub struct RecoveryManager {
    app_data_dir: PathBuf,
}

impl RecoveryManager {
    pub fn new(app_data_dir: PathBuf) -> Self {
        Self { app_data_dir }
    }

    pub fn sentinel_path(&self) -> PathBuf {
        self.app_data_dir.join("running.sentinel")
    }

    pub fn autosave_dir(&self) -> PathBuf {
        self.app_data_dir.join("autosave")
    }

    pub fn mark_app_started(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.app_data_dir)?;
        let mut f = std::fs::File::create(self.sentinel_path())?;
        use std::io::Write;
        writeln!(f, "PID: {}", std::process::id())?;
        Ok(())
    }

    pub fn had_unclean_shutdown(&self) -> bool {
        self.sentinel_path().exists()
    }

    pub fn mark_app_stopped(&self) -> std::io::Result<()> {
        let p = self.sentinel_path();
        if p.exists() {
            std::fs::remove_file(p)?;
        }
        Ok(())
    }

    pub fn clear_all_snapshots(&self) -> std::io::Result<()> {
        let dir = self.autosave_dir();
        if dir.exists() {
            for entry in std::fs::read_dir(&dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.is_file() {
                    let _ = std::fs::remove_file(path);
                }
            }
        }
        self.mark_app_stopped()
    }

    pub fn check_recovery_needed(&self) -> Vec<RecoveredDoc> {
        let sentinel = self.sentinel_path();
        if !sentinel.exists() {
            return Vec::new();
        }

        let dir = self.autosave_dir();
        if !dir.exists() {
            return Vec::new();
        }

        let mut list = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.extension().is_some_and(|ext| ext == "redoc") {
                    let ts = entry
                        .metadata()
                        .ok()
                        .and_then(|m| m.modified().ok())
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|d| d.as_secs())
                        .unwrap_or_default();

                    let name = p
                        .file_stem()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_default();
                    
                    let mut title = format!("Recovered {}", name);
                    let mut mode = "doc".to_string();

                    if let Ok(file) = std::fs::File::open(&p) {
                        if let Ok(mut archive) = zip::ZipArchive::new(file) {
                            if let Ok(mut meta_file) = archive.by_name("meta.json") {
                                use std::io::Read;
                                let mut meta_str = String::new();
                                if meta_file.read_to_string(&mut meta_str).is_ok() {
                                    if let Ok(meta_json) = serde_json::from_str::<serde_json::Value>(&meta_str) {
                                        if let Some(t) = meta_json.get("title").and_then(|v| v.as_str()) {
                                            title = t.to_string();
                                        }
                                        if let Some(m) = meta_json.get("mode").and_then(|v| v.as_str()) {
                                            mode = m.to_string();
                                        }
                                    }
                                }
                            } else {
                                // Missing meta.json
                                continue;
                            }
                        } else {
                            // Corrupt ZIP
                            continue;
                        }
                    } else {
                        continue;
                    }

                    list.push(RecoveredDoc {
                        id: name.clone(),
                        title,
                        mode,
                        path: None,
                        snapshot_path: p.to_string_lossy().to_string(),
                        timestamp: ts,
                    });
                }
            }
        }
        list
    }
}
