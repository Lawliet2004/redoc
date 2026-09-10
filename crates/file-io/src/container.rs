// SPDX-License-Identifier: MIT OR Apache-2.0
use redoc_core::sanitize_author_name;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use specta::Type;
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use thiserror::Error;

pub const CURRENT_FORMAT_VERSION: u32 = 2;
pub const MAX_ARCHIVE_ENTRIES: usize = 10_000;
pub const MAX_JSON_ENTRY_BYTES: u64 = 16 * 1024 * 1024;
pub const MAX_ASSET_BYTES: u64 = 32 * 1024 * 1024;
pub const MAX_TOTAL_ASSET_BYTES: u64 = 256 * 1024 * 1024;

struct TempPathGuard(Option<PathBuf>);

impl TempPathGuard {
    fn new(path: PathBuf) -> Self {
        Self(Some(path))
    }

    fn disarm(&mut self) {
        self.0 = None;
    }
}

impl Drop for TempPathGuard {
    fn drop(&mut self) {
        if let Some(path) = self.0.take() {
            let _ = std::fs::remove_file(path);
        }
    }
}

#[derive(Error, Debug)]
pub enum FileIoError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("ZIP error: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Invalid container: missing {0}")]
    InvalidContainer(&'static str),
    #[error("Unsupported format version: {0}")]
    UnsupportedVersion(u32),
    #[error("Migration error: {0}")]
    Migration(String),
    #[error("Archive has too many entries (maximum {0})")]
    TooManyEntries(usize),
    #[error("Archive entry {name} exceeds the {limit} byte limit")]
    EntryTooLarge { name: String, limit: u64 },
    #[error("Invalid asset reference: {0}")]
    InvalidAsset(String),
    #[error("Asset {hash} declares {declared} bytes but contains {actual}")]
    AssetSizeMismatch {
        hash: String,
        declared: u64,
        actual: u64,
    },
    #[error("Asset hash mismatch for {hash}")]
    AssetHashMismatch { hash: String },
    #[error("Invalid document id: {0}")]
    InvalidDocId(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AssetInfo {
    pub hash: String,
    pub mime: String,
    pub name: String,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RedocMeta {
    pub format_version: u32,
    pub id: String,
    pub mode: String, // "doc" | "sheet" | "slide"
    pub title: String,
    pub created_at: u64,
    pub updated_at: u64,
    pub author: Option<String>,
    /// Bounded list of collaborator display names that edited this file
    /// on this device (local-first attribution, no server account).
    #[serde(default)]
    pub collaborators: Vec<String>,
    /// Last-writer-wins marker: unix seconds + author of the winning write.
    /// Readers use it for the merge prompt / history drawer ordering.
    #[serde(default)]
    pub revision: u64,
    #[serde(default)]
    pub last_modified_by: Option<String>,
    /// Simple per-document capability list, e.g. `["read","comment"]`
    /// (absence of `"write"` renders the document read-only in the shell).
    #[serde(default)]
    pub permissions: Vec<String>,
    pub app_version: String,
    pub assets: Vec<AssetInfo>,
    pub dirty_on_crash: Option<bool>,
    #[serde(default)]
    pub read_only: Option<bool>,
    #[serde(default)]
    pub warning: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedocContainer {
    pub meta: RedocMeta,
    pub body: serde_json::Value,
    pub assets_data: std::collections::HashMap<String, Vec<u8>>, // hash -> bytes
    #[serde(default)]
    pub repair: ContainerRepairReport,
}

/// Friendly, non-fatal report attached to a container read: missing assets,
/// future-version notices, and migration notes are surfaced inline instead
/// of silently rejecting the document.
#[derive(Debug, Clone, Default, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ContainerRepairReport {
    pub warnings: Vec<String>,
    pub read_only: bool,
    pub migrated: bool,
    pub missing_assets: Vec<String>,
}

impl RedocContainer {
    pub fn new(mode: &str, title: &str, body: serde_json::Value) -> Self {
        Self::new_with_author(mode, title, body, None)
    }

    /// Create a container attributed to `author` (local-first identity).
    /// `None` preserves the legacy `author: None` shape for older tests.
    pub fn new_with_author(
        mode: &str,
        title: &str,
        body: serde_json::Value,
        author: Option<&str>,
    ) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let attributed = author.map(sanitize_author_name);
        let meta = RedocMeta {
            format_version: CURRENT_FORMAT_VERSION,
            id: uuid::Uuid::now_v7().to_string(),
            mode: mode.to_string(),
            title: title.to_string(),
            created_at: now,
            updated_at: now,
            author: attributed.clone(),
            collaborators: attributed.clone().into_iter().collect(),
            revision: now,
            last_modified_by: attributed,
            permissions: vec![
                "read".to_string(),
                "write".to_string(),
                "comment".to_string(),
            ],
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            assets: Vec::new(),
            dirty_on_crash: Some(false),
            read_only: None,
            warning: None,
        };

        Self {
            meta,
            body,
            assets_data: std::collections::HashMap::new(),
            repair: ContainerRepairReport::default(),
        }
    }

    pub fn add_asset(&mut self, name: &str, mime: &str, data: Vec<u8>) -> String {
        let mut hasher = Sha256::new();
        hasher.update(&data);
        let hash = format!("{:x}", hasher.finalize());

        if !self.assets_data.contains_key(&hash) {
            let size = data.len() as u64;
            self.assets_data.insert(hash.clone(), data);
            self.meta.assets.push(AssetInfo {
                hash: hash.clone(),
                mime: mime.to_string(),
                name: name.to_string(),
                size,
            });
        }
        hash
    }

    pub fn read_from_file<P: AsRef<Path>>(path: P) -> Result<Self, FileIoError> {
        let file = File::open(path)?;
        let mut archive = zip::ZipArchive::new(file)?;
        if archive.len() > MAX_ARCHIVE_ENTRIES {
            return Err(FileIoError::TooManyEntries(MAX_ARCHIVE_ENTRIES));
        }

        // Read meta.json
        let mut meta: RedocMeta = {
            let mut meta_file = archive
                .by_name("meta.json")
                .map_err(|_| FileIoError::InvalidContainer("meta.json"))?;
            if meta_file.size() > MAX_JSON_ENTRY_BYTES {
                return Err(FileIoError::EntryTooLarge {
                    name: "meta.json".to_string(),
                    limit: MAX_JSON_ENTRY_BYTES,
                });
            }
            let mut content = String::new();
            meta_file.read_to_string(&mut content)?;
            serde_json::from_str(&content)?
        };

        // Read body.json
        let mut body: serde_json::Value = {
            let mut body_file = archive
                .by_name("body.json")
                .map_err(|_| FileIoError::InvalidContainer("body.json"))?;
            if body_file.size() > MAX_JSON_ENTRY_BYTES {
                return Err(FileIoError::EntryTooLarge {
                    name: "body.json".to_string(),
                    limit: MAX_JSON_ENTRY_BYTES,
                });
            }
            let mut content = String::new();
            body_file.read_to_string(&mut content)?;
            serde_json::from_str(&content)?
        };

        let original_format_version = meta.format_version;
        if meta.format_version > CURRENT_FORMAT_VERSION {
            meta.read_only = Some(true);
            meta.warning = Some(format!(
                "Document format version {} is newer than current supported version {}",
                meta.format_version, CURRENT_FORMAT_VERSION
            ));
        } else if meta.format_version < CURRENT_FORMAT_VERSION {
            body = crate::migrations::migrate_version(
                body,
                meta.format_version,
                CURRENT_FORMAT_VERSION,
            )
            .map_err(FileIoError::Migration)?;
            meta.format_version = CURRENT_FORMAT_VERSION;
        }

        // Read assets
        let mut assets_data = std::collections::HashMap::new();
        let mut missing_assets = Vec::new();
        let mut total_asset_bytes = 0u64;
        for asset in &meta.assets {
            if asset.hash.is_empty()
                || asset.hash.contains('/')
                || asset.hash.contains('\\')
                || asset.hash.contains("..")
                || asset.hash.bytes().any(|byte| byte == 0)
            {
                return Err(FileIoError::InvalidAsset(asset.hash.clone()));
            }
            if asset.size > MAX_ASSET_BYTES {
                return Err(FileIoError::EntryTooLarge {
                    name: format!("assets/{}", asset.hash),
                    limit: MAX_ASSET_BYTES,
                });
            }
            total_asset_bytes =
                total_asset_bytes
                    .checked_add(asset.size)
                    .ok_or(FileIoError::EntryTooLarge {
                        name: "assets/*".to_string(),
                        limit: MAX_TOTAL_ASSET_BYTES,
                    })?;
            if total_asset_bytes > MAX_TOTAL_ASSET_BYTES {
                return Err(FileIoError::EntryTooLarge {
                    name: "assets/*".to_string(),
                    limit: MAX_TOTAL_ASSET_BYTES,
                });
            }
            let asset_path = format!("assets/{}", asset.hash);
            if let Ok(mut asset_file) = archive.by_name(&asset_path) {
                if asset.hash.len() != 64
                    || !asset.hash.bytes().all(|byte| byte.is_ascii_hexdigit())
                {
                    return Err(FileIoError::InvalidAsset(asset.hash.clone()));
                }
                if asset_file.size() > MAX_ASSET_BYTES {
                    return Err(FileIoError::EntryTooLarge {
                        name: asset_path,
                        limit: MAX_ASSET_BYTES,
                    });
                }
                let mut data = Vec::new();
                asset_file.read_to_end(&mut data)?;
                if data.len() as u64 != asset.size {
                    return Err(FileIoError::AssetSizeMismatch {
                        hash: asset.hash.clone(),
                        declared: asset.size,
                        actual: data.len() as u64,
                    });
                }
                let mut hasher = Sha256::new();
                hasher.update(&data);
                if format!("{:x}", hasher.finalize()) != asset.hash {
                    return Err(FileIoError::AssetHashMismatch {
                        hash: asset.hash.clone(),
                    });
                }
                assets_data.insert(asset.hash.clone(), data);
            } else {
                missing_assets.push(asset.hash.clone());
            }
        }

        let mut warnings = Vec::new();
        let mut read_only = false;
        let mut migrated = false;
        if !missing_assets.is_empty() {
            warnings.push(format!(
                "{} listed asset(s) are missing from the archive",
                missing_assets.len()
            ));
        }
        if original_format_version > CURRENT_FORMAT_VERSION {
            read_only = true;
        }
        if original_format_version < CURRENT_FORMAT_VERSION {
            migrated = true;
        }
        if let Some(warning) = &meta.warning {
            if !warning.is_empty() && !warnings.iter().any(|w| w == warning) {
                warnings.push(warning.clone());
            }
        }

        Ok(Self {
            meta,
            body,
            assets_data,
            repair: ContainerRepairReport {
                warnings,
                read_only,
                migrated,
                missing_assets,
            },
        })
    }

    pub fn save_atomic<P: AsRef<Path>>(&mut self, path: P) -> Result<(), FileIoError> {
        let target_path = path.as_ref();
        let parent = target_path.parent().unwrap_or_else(|| Path::new("."));
        std::fs::create_dir_all(parent)?;
        let temp_filename = format!(".tmp_{}", uuid::Uuid::now_v7());
        let temp_path = parent.join(temp_filename);
        let mut temp_guard = TempPathGuard::new(temp_path.clone());

        self.meta.updated_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        {
            let file = File::create(&temp_path)?;
            let mut zip = zip::ZipWriter::new(file);
            let options = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated);

            // Write meta.json
            zip.start_file("meta.json", options)?;
            let meta_json = serde_json::to_string_pretty(&self.meta)?;
            zip.write_all(meta_json.as_bytes())?;

            // Write body.json
            zip.start_file("body.json", options)?;
            let body_json = serde_json::to_string(&self.body)?;
            zip.write_all(body_json.as_bytes())?;

            // Write assets
            for (hash, data) in &self.assets_data {
                let asset_path = format!("assets/{}", hash);
                zip.start_file(&asset_path, options)?;
                zip.write_all(data)?;
            }

            let file = zip.finish()?;
            file.sync_all()?;
        }

        // Rename temp to target atomically or via backup
        if let Err(err) = std::fs::rename(&temp_path, target_path) {
            if target_path.exists() {
                let target_name = target_path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("document");
                let backup_path =
                    parent.join(format!(".{}.bak-{}", target_name, uuid::Uuid::now_v7()));
                std::fs::rename(target_path, &backup_path)?;
                if let Err(rename_err) = std::fs::rename(&temp_path, target_path) {
                    if let Err(restore_err) = std::fs::rename(&backup_path, target_path) {
                        return Err(FileIoError::Io(std::io::Error::new(
                            restore_err.kind(),
                            format!(
                                "atomic replace failed ({rename_err}); original is recoverable at {}",
                                backup_path.display()
                            ),
                        )));
                    }
                    return Err(rename_err.into());
                }
                let _ = std::fs::remove_file(backup_path);
            } else {
                return Err(err.into());
            }
        }

        temp_guard.disarm();
        Ok(())
    }
}

/// Writes arbitrary exported bytes without truncating an existing destination.
/// The temporary file is fsynced before replacement and cleaned up on every
/// error path. On platforms that cannot atomically replace an existing file,
/// the original is moved aside until the replacement succeeds.
pub fn write_bytes_atomic<P: AsRef<Path>>(path: P, bytes: &[u8]) -> Result<(), FileIoError> {
    let target_path = path.as_ref();
    let parent = target_path.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent)?;
    let temp_path = parent.join(format!(".tmp_export_{}", uuid::Uuid::now_v7()));
    let mut temp_guard = TempPathGuard::new(temp_path.clone());
    {
        let mut file = File::create(&temp_path)?;
        file.write_all(bytes)?;
        file.sync_all()?;
    }

    if let Err(err) = std::fs::rename(&temp_path, target_path) {
        if !target_path.exists() {
            return Err(err.into());
        }
        let target_name = target_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("export");
        let backup_path = parent.join(format!(".{}.bak-{}", target_name, uuid::Uuid::now_v7()));
        std::fs::rename(target_path, &backup_path)?;
        if let Err(rename_err) = std::fs::rename(&temp_path, target_path) {
            if let Err(restore_err) = std::fs::rename(&backup_path, target_path) {
                return Err(FileIoError::Io(std::io::Error::new(
                    restore_err.kind(),
                    format!(
                        "atomic export failed ({rename_err}); original is recoverable at {}",
                        backup_path.display()
                    ),
                )));
            }
            return Err(rename_err.into());
        }
        let _ = std::fs::remove_file(backup_path);
    }
    temp_guard.disarm();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_body_and_assets() {
        let path =
            std::env::temp_dir().join(format!("redoc-roundtrip-{}.redoc", uuid::Uuid::now_v7()));
        let mut original = RedocContainer::new(
            "doc",
            "Round trip",
            serde_json::json!({ "type": "doc", "content": [{ "type": "paragraph" }] }),
        );
        let hash = original.add_asset("pixel.bin", "application/octet-stream", vec![1, 2, 3]);
        original.save_atomic(&path).expect("save container");

        let loaded = RedocContainer::read_from_file(&path).expect("read container");
        assert_eq!(loaded.meta.id, original.meta.id);
        assert_eq!(loaded.body, original.body);
        assert_eq!(loaded.assets_data.get(&hash), Some(&vec![1, 2, 3]));
        std::fs::remove_file(path).expect("cleanup test container");
    }

    #[test]
    fn data_uri_images_are_not_duplicated_into_assets() {
        let path =
            std::env::temp_dir().join(format!("redoc-inline-{}.redoc", uuid::Uuid::now_v7()));
        let mut container = RedocContainer::new(
            "slide",
            "Image",
            serde_json::json!({ "src": "data:image/png;base64,aGVsbG8=" }),
        );
        container.save_atomic(&path).expect("save container");

        let loaded = RedocContainer::read_from_file(&path).expect("read container");
        assert!(
            loaded.meta.assets.is_empty(),
            "data-URI images must not be copied into assets/"
        );
        assert_eq!(
            loaded.body,
            serde_json::json!({ "src": "data:image/png;base64,aGVsbG8=" })
        );
        std::fs::remove_file(path).expect("cleanup test container");
    }

    #[test]
    fn test_save_atomic_overwrites_existing_file() {
        let path =
            std::env::temp_dir().join(format!("redoc-overwrite-{}.redoc", uuid::Uuid::now_v7()));
        let mut original = RedocContainer::new(
            "doc",
            "Initial Document",
            serde_json::json!({ "type": "doc", "content": [] }),
        );
        original.save_atomic(&path).expect("initial save failed");

        let mut updated = RedocContainer::new(
            "doc",
            "Updated Document",
            serde_json::json!({ "type": "doc", "content": [{ "type": "paragraph" }] }),
        );
        updated.save_atomic(&path).expect("overwrite save failed");

        let loaded = RedocContainer::read_from_file(&path).expect("read container after overwrite");
        assert_eq!(loaded.meta.title, "Updated Document");
        std::fs::remove_file(path).expect("cleanup test container");
    }

    #[test]
    fn write_bytes_atomic_overwrites_without_leaving_temp_files() {
        let dir = std::env::temp_dir().join(format!("redoc-export-bytes-{}", uuid::Uuid::now_v7()));
        let path = dir.join("export.bin");
        write_bytes_atomic(&path, b"first").expect("initial atomic write");
        write_bytes_atomic(&path, b"second").expect("replacement atomic write");
        assert_eq!(std::fs::read(&path).expect("read atomic output"), b"second");
        let prefix = ".tmp_export_";
        let leftovers = std::fs::read_dir(&dir)
            .expect("read temp directory")
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().starts_with(prefix))
            .count();
        assert_eq!(leftovers, 0, "atomic writes left temporary files behind");
        std::fs::remove_dir_all(dir).expect("cleanup atomic output");
    }

    #[test]
    fn test_schema_migration_runs_on_legacy_container() {
        let path =
            std::env::temp_dir().join(format!("redoc-legacy-{}.redoc", uuid::Uuid::now_v7()));
        let mut original = RedocContainer::new(
            "doc",
            "Legacy Document",
            serde_json::json!({ "type": "doc", "content": [] }),
        );
        original.meta.format_version = 0;
        original.save_atomic(&path).expect("save legacy container");

        let loaded = RedocContainer::read_from_file(&path).expect("read legacy container");
        assert_eq!(loaded.meta.format_version, CURRENT_FORMAT_VERSION);
        assert_ne!(loaded.meta.read_only, Some(true));
        std::fs::remove_file(path).expect("cleanup test container");
    }

    #[test]
    fn test_newer_version_container_sets_read_only_and_warning() {
        let path =
            std::env::temp_dir().join(format!("redoc-future-{}.redoc", uuid::Uuid::now_v7()));
        let mut original = RedocContainer::new(
            "doc",
            "Future Document",
            serde_json::json!({ "type": "doc", "content": [] }),
        );
        original.meta.format_version = 999;
        original.save_atomic(&path).expect("save future container");

        let loaded = RedocContainer::read_from_file(&path).expect("read future container");
        assert_eq!(loaded.meta.read_only, Some(true));
        assert!(loaded.meta.warning.is_some());
        assert!(loaded.repair.read_only);
        assert!(!loaded.repair.warnings.is_empty());
        std::fs::remove_file(path).expect("cleanup test container");
    }

    #[test]
    fn missing_listed_assets_surface_in_repair_report() {
        let path =
            std::env::temp_dir().join(format!("redoc-missing-{}.redoc", uuid::Uuid::now_v7()));
        let mut original = RedocContainer::new(
            "doc",
            "Missing Asset",
            serde_json::json!({ "type": "doc", "content": [] }),
        );
        original.meta.assets.push(AssetInfo {
            hash: "f".repeat(64),
            mime: "image/png".to_string(),
            name: "gone.png".to_string(),
            size: 7,
        });
        original.save_atomic(&path).expect("save container");

        let loaded = RedocContainer::read_from_file(&path).expect("read container");
        assert_eq!(loaded.repair.missing_assets, vec!["f".repeat(64)]);
        assert!(!loaded.repair.warnings.is_empty());
        assert!(!loaded.repair.read_only);
        std::fs::remove_file(path).expect("cleanup test container");
    }
}
