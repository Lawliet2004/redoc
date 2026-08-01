use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use specta::Type;
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;
use thiserror::Error;

pub const CURRENT_FORMAT_VERSION: u32 = 1;

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
}

impl RedocContainer {
    pub fn new(mode: &str, title: &str, body: serde_json::Value) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let meta = RedocMeta {
            format_version: CURRENT_FORMAT_VERSION,
            id: uuid::Uuid::now_v7().to_string(),
            mode: mode.to_string(),
            title: title.to_string(),
            created_at: now,
            updated_at: now,
            author: None,
            app_version: "0.1.0".to_string(),
            assets: Vec::new(),
            dirty_on_crash: Some(false),
            read_only: None,
            warning: None,
        };

        Self {
            meta,
            body,
            assets_data: std::collections::HashMap::new(),
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

    pub fn add_inline_data_uri_assets(&mut self) {
        fn collect(value: &serde_json::Value, assets: &mut Vec<(String, String, Vec<u8>)>) {
            match value {
                serde_json::Value::String(text) if text.starts_with("data:") => {
                    if let Some((header, encoded)) = text.split_once(',') {
                        let mime = header
                            .strip_prefix("data:")
                            .and_then(|value| value.split(';').next())
                            .unwrap_or("application/octet-stream")
                            .to_string();
                        if let Some(bytes) = decode_base64(encoded) {
                            assets.push(("inline-asset".to_string(), mime, bytes));
                        }
                    }
                }
                serde_json::Value::Array(values) => {
                    for value in values {
                        collect(value, assets);
                    }
                }
                serde_json::Value::Object(values) => {
                    for value in values.values() {
                        collect(value, assets);
                    }
                }
                _ => {}
            }
        }

        let mut assets = Vec::new();
        collect(&self.body, &mut assets);
        for (name, mime, data) in assets {
            self.add_asset(&name, &mime, data);
        }
    }

    pub fn read_from_file<P: AsRef<Path>>(path: P) -> Result<Self, FileIoError> {
        let file = File::open(path)?;
        let mut archive = zip::ZipArchive::new(file)?;

        // Read meta.json
        let mut meta: RedocMeta = {
            let mut meta_file = archive
                .by_name("meta.json")
                .map_err(|_| FileIoError::InvalidContainer("meta.json"))?;
            let mut content = String::new();
            meta_file.read_to_string(&mut content)?;
            serde_json::from_str(&content)?
        };

        // Read body.json
        let mut body: serde_json::Value = {
            let mut body_file = archive
                .by_name("body.json")
                .map_err(|_| FileIoError::InvalidContainer("body.json"))?;
            let mut content = String::new();
            body_file.read_to_string(&mut content)?;
            serde_json::from_str(&content)?
        };

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
        for asset in &meta.assets {
            let asset_path = format!("assets/{}", asset.hash);
            if let Ok(mut asset_file) = archive.by_name(&asset_path) {
                let mut data = Vec::new();
                asset_file.read_to_end(&mut data)?;
                assets_data.insert(asset.hash.clone(), data);
            }
        }

        Ok(Self {
            meta,
            body,
            assets_data,
        })
    }

    pub fn save_atomic<P: AsRef<Path>>(&mut self, path: P) -> Result<(), FileIoError> {
        let target_path = path.as_ref();
        let parent = target_path.parent().unwrap_or_else(|| Path::new("."));
        std::fs::create_dir_all(parent)?;
        let temp_filename = format!(".tmp_{}", uuid::Uuid::now_v7());
        let temp_path = parent.join(temp_filename);

        self.meta.updated_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        {
            let file = File::create(&temp_path)?;
            let mut zip = zip::ZipWriter::new(file);
            let options = zip::write::FileOptions::default()
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

        // Rename temp to target atomically
        if let Err(err) = std::fs::rename(&temp_path, target_path) {
            if target_path.exists() {
                let _ = std::fs::remove_file(target_path);
                std::fs::rename(&temp_path, target_path)?;
            } else {
                return Err(err.into());
            }
        }

        Ok(())
    }
}

fn decode_base64(value: &str) -> Option<Vec<u8>> {
    let mut output = Vec::new();
    let mut buffer = 0u32;
    let mut bits = 0u8;
    for byte in value.bytes().filter(|byte| !byte.is_ascii_whitespace()) {
        if byte == b'=' {
            break;
        }
        let value = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return None,
        } as u32;
        buffer = (buffer << 6) | value;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            output.push((buffer >> bits) as u8);
            buffer &= (1 << bits) - 1;
        }
    }
    Some(output)
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
    fn indexes_inline_data_uri_assets() {
        let mut container = RedocContainer::new(
            "slide",
            "Image",
            serde_json::json!({ "src": "data:image/png;base64,aGVsbG8=" }),
        );
        container.add_inline_data_uri_assets();
        assert_eq!(container.meta.assets.len(), 1);
        assert_eq!(container.meta.assets[0].mime, "image/png");
        assert_eq!(
            container.assets_data.get(&container.meta.assets[0].hash),
            Some(&b"hello".to_vec())
        );
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
        std::fs::remove_file(path).expect("cleanup test container");
    }
}
