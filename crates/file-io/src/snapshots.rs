// SPDX-License-Identifier: MIT OR Apache-2.0
use crate::container::{FileIoError, RedocContainer};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::PathBuf;

fn compute_body_hash(body: &serde_json::Value) -> String {
    let json_bytes = serde_json::to_vec(body).unwrap_or_default();
    let mut hasher = Sha256::new();
    hasher.update(&json_bytes);
    format!("{:x}", hasher.finalize())
}

pub struct SnapshotManager {
    autosave_dir: PathBuf,
}

/// One entry in the local-first version history: a rotated `.bak*`
/// snapshot plus the attribution captured in its container meta.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    pub slot: String,
    pub path: String,
    pub title: String,
    pub author: Option<String>,
    pub last_modified_by: Option<String>,
    pub updated_at: u64,
    pub revision: u64,
    pub body_hash: String,
}

/// Last-write-wins comparison between the on-disk file and a snapshot.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct MergeDecision {
    pub winner: String,
    pub current_revision: u64,
    pub candidate_revision: u64,
    pub current_hash: String,
    pub candidate_hash: String,
    pub changed: bool,
}

fn sanitize_snapshot_id(doc_id: &str) -> Result<&str, FileIoError> {
    if doc_id.is_empty()
        || doc_id.len() > 80
        || !doc_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
    {
        return Err(FileIoError::InvalidDocId(doc_id.to_string()));
    }
    Ok(doc_id)
}

impl SnapshotManager {
    pub fn new(autosave_dir: PathBuf) -> Self {
        Self { autosave_dir }
    }

    fn snapshot_file(&self, doc_id: &str, suffix: &str) -> Result<PathBuf, FileIoError> {
        let id = sanitize_snapshot_id(doc_id)?;
        let name = format!("{id}{suffix}");
        let path = self.autosave_dir.join(&name);
        if path.file_name().and_then(|name| name.to_str()) != Some(name.as_str()) {
            return Err(FileIoError::InvalidDocId(doc_id.to_string()));
        }
        Ok(path)
    }

    pub fn write_snapshot(
        &self,
        doc_id: &str,
        container: &mut RedocContainer,
    ) -> Result<PathBuf, FileIoError> {
        std::fs::create_dir_all(&self.autosave_dir)?;
        let snapshot_file = self.snapshot_file(doc_id, ".redoc")?;

        // Hash-gate autosave: skip writing snapshot if hash hasn't changed
        if snapshot_file.exists() {
            if let Ok(existing_container) = RedocContainer::read_from_file(&snapshot_file) {
                let current_hash = compute_body_hash(&container.body);
                let existing_hash = compute_body_hash(&existing_container.body);
                if current_hash == existing_hash {
                    return Ok(snapshot_file);
                }
            }
        }

        // 3-deep snapshot rotation (.bak3 <- .bak2 <- .bak1 <- primary)
        let bak1 = self.snapshot_file(doc_id, ".redoc.bak1")?;
        let bak2 = self.snapshot_file(doc_id, ".redoc.bak2")?;
        let bak3 = self.snapshot_file(doc_id, ".redoc.bak3")?;

        if bak2.exists() {
            let _ = std::fs::rename(&bak2, &bak3);
        }
        if bak1.exists() {
            let _ = std::fs::rename(&bak1, &bak2);
        }
        if snapshot_file.exists() {
            let _ = std::fs::rename(&snapshot_file, &bak1);
        }

        container.save_atomic(&snapshot_file)?;
        Ok(snapshot_file)
    }

    pub fn remove_snapshot(&self, doc_id: &str) -> std::io::Result<()> {
        let Ok(snapshot_file) = self.snapshot_file(doc_id, ".redoc") else {
            return Ok(());
        };
        if snapshot_file.exists() {
            let _ = std::fs::remove_file(snapshot_file);
        }
        for i in 1..=3 {
            if let Ok(bak) = self.snapshot_file(doc_id, &format!(".redoc.bak{i}")) {
                if bak.exists() {
                    let _ = std::fs::remove_file(bak);
                }
            }
        }
        Ok(())
    }

    fn history_slot(&self, doc_id: &str, slot: &str) -> Option<HistoryEntry> {
        let file = match slot {
            "current" => self.snapshot_file(doc_id, ".redoc").ok()?,
            "bak1" => self.snapshot_file(doc_id, ".redoc.bak1").ok()?,
            "bak2" => self.snapshot_file(doc_id, ".redoc.bak2").ok()?,
            "bak3" => self.snapshot_file(doc_id, ".redoc.bak3").ok()?,
            _ => return None,
        };
        if !file.exists() {
            return None;
        }
        let container = RedocContainer::read_from_file(&file).ok()?;
        Some(HistoryEntry {
            slot: slot.to_string(),
            path: file.to_string_lossy().to_string(),
            title: container.meta.title.clone(),
            author: container.meta.author.clone(),
            last_modified_by: container.meta.last_modified_by.clone(),
            updated_at: container.meta.updated_at,
            revision: container.meta.revision,
            body_hash: compute_body_hash(&container.body),
        })
    }

    /// Version-history drawer source: current snapshot + rotated backups,
    /// newest slot first, skipping missing or unreadable slots.
    pub fn list_history(&self, doc_id: &str) -> Vec<HistoryEntry> {
        ["current", "bak1", "bak2", "bak3"]
            .iter()
            .filter_map(|slot| self.history_slot(doc_id, slot))
            .collect()
    }

    /// Last-write-wins comparison: the higher `revision` (falling back to
    /// `updated_at`) wins; equal revisions with different hashes surface a
    /// merge prompt instead of silently overwriting.
    pub fn merge_decision(current: &RedocContainer, candidate: &RedocContainer) -> MergeDecision {
        let current_stamp = current.meta.revision.max(current.meta.updated_at);
        let candidate_stamp = candidate.meta.revision.max(candidate.meta.updated_at);
        let current_hash = compute_body_hash(&current.body);
        let candidate_hash = compute_body_hash(&candidate.body);
        let changed = current_hash != candidate_hash;
        let winner = if candidate_stamp > current_stamp {
            "candidate"
        } else {
            "current"
        }
        .to_string();
        MergeDecision {
            winner,
            current_revision: current_stamp,
            candidate_revision: candidate_stamp,
            current_hash,
            candidate_hash,
            changed,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_snapshot_rotation_3_deep() {
        let dir = std::env::temp_dir().join(format!("redoc-snapshots-{}", uuid::Uuid::now_v7()));
        let manager = SnapshotManager::new(dir.clone());
        let doc_id = "doc123";

        let primary = dir.join(format!("{}.redoc", doc_id));
        let bak1 = dir.join(format!("{}.redoc.bak1", doc_id));
        let bak2 = dir.join(format!("{}.redoc.bak2", doc_id));
        let bak3 = dir.join(format!("{}.redoc.bak3", doc_id));

        // 1. Initial snapshot (v1)
        let mut c1 = RedocContainer::new("doc", "V1", json!({ "v": 1 }));
        manager.write_snapshot(doc_id, &mut c1).expect("write 1");
        assert!(primary.exists());
        assert!(!bak1.exists());

        // 2. Write identical content (v1) -> Hash-gate should skip saving/rotation
        let mut c1_dup = RedocContainer::new("doc", "V1", json!({ "v": 1 }));
        manager
            .write_snapshot(doc_id, &mut c1_dup)
            .expect("write 1 dup");
        assert!(primary.exists());
        assert!(!bak1.exists());

        // 3. Write snapshot (v2) -> rotates primary(v1) to bak1
        let mut c2 = RedocContainer::new("doc", "V2", json!({ "v": 2 }));
        manager.write_snapshot(doc_id, &mut c2).expect("write 2");
        assert!(primary.exists());
        assert!(bak1.exists());
        assert!(!bak2.exists());
        assert_eq!(
            RedocContainer::read_from_file(&primary).unwrap().body,
            json!({ "v": 2 })
        );
        assert_eq!(
            RedocContainer::read_from_file(&bak1).unwrap().body,
            json!({ "v": 1 })
        );

        // 4. Write snapshot (v3) -> bak1(v1) to bak2
        let mut c3 = RedocContainer::new("doc", "V3", json!({ "v": 3 }));
        manager.write_snapshot(doc_id, &mut c3).expect("write 3");
        assert!(bak2.exists());
        assert!(!bak3.exists());
        assert_eq!(
            RedocContainer::read_from_file(&bak2).unwrap().body,
            json!({ "v": 1 })
        );

        // 5. Write snapshot (v4) -> bak2(v1) to bak3
        let mut c4 = RedocContainer::new("doc", "V4", json!({ "v": 4 }));
        manager.write_snapshot(doc_id, &mut c4).expect("write 4");
        assert!(bak3.exists());
        assert_eq!(
            RedocContainer::read_from_file(&bak3).unwrap().body,
            json!({ "v": 1 })
        );
        assert_eq!(
            RedocContainer::read_from_file(&bak2).unwrap().body,
            json!({ "v": 2 })
        );
        assert_eq!(
            RedocContainer::read_from_file(&bak1).unwrap().body,
            json!({ "v": 3 })
        );
        assert_eq!(
            RedocContainer::read_from_file(&primary).unwrap().body,
            json!({ "v": 4 })
        );

        // 6. Write snapshot (v5) -> v1 is dropped, bak3 becomes v2
        let mut c5 = RedocContainer::new("doc", "V5", json!({ "v": 5 }));
        manager.write_snapshot(doc_id, &mut c5).expect("write 5");
        assert_eq!(
            RedocContainer::read_from_file(&bak3).unwrap().body,
            json!({ "v": 2 })
        );
        assert_eq!(
            RedocContainer::read_from_file(&bak2).unwrap().body,
            json!({ "v": 3 })
        );
        assert_eq!(
            RedocContainer::read_from_file(&bak1).unwrap().body,
            json!({ "v": 4 })
        );
        assert_eq!(
            RedocContainer::read_from_file(&primary).unwrap().body,
            json!({ "v": 5 })
        );

        // Cleanup
        manager.remove_snapshot(doc_id).expect("remove snapshots");
        assert!(!primary.exists());
        assert!(!bak1.exists());
        assert!(!bak2.exists());
        assert!(!bak3.exists());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn rejects_path_traversal_document_ids() {
        let dir =
            std::env::temp_dir().join(format!("redoc-snapshots-safe-{}", uuid::Uuid::now_v7()));
        std::fs::create_dir_all(&dir).expect("create snapshot dir");
        let manager = SnapshotManager::new(dir.clone());
        let mut container = RedocContainer::new("doc", "Bad", json!({ "v": 1 }));
        let error = manager
            .write_snapshot("../escape", &mut container)
            .expect_err("traversal id must fail");
        assert!(matches!(error, FileIoError::InvalidDocId(_)));
        assert!(std::fs::read_dir(&dir).expect("read dir").next().is_none());
        let _ = std::fs::remove_dir_all(dir);
    }
}
