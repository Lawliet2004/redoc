use crate::container::{FileIoError, RedocContainer};
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

impl SnapshotManager {
    pub fn new(autosave_dir: PathBuf) -> Self {
        Self { autosave_dir }
    }

    pub fn write_snapshot(
        &self,
        doc_id: &str,
        container: &mut RedocContainer,
    ) -> Result<PathBuf, FileIoError> {
        std::fs::create_dir_all(&self.autosave_dir)?;
        let snapshot_file = self.autosave_dir.join(format!("{}.redoc", doc_id));

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
        let bak1 = self.autosave_dir.join(format!("{}.redoc.bak1", doc_id));
        let bak2 = self.autosave_dir.join(format!("{}.redoc.bak2", doc_id));
        let bak3 = self.autosave_dir.join(format!("{}.redoc.bak3", doc_id));

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
        let snapshot_file = self.autosave_dir.join(format!("{}.redoc", doc_id));
        if snapshot_file.exists() {
            let _ = std::fs::remove_file(snapshot_file);
        }
        for i in 1..=3 {
            let bak = self.autosave_dir.join(format!("{}.redoc.bak{}", doc_id, i));
            if bak.exists() {
                let _ = std::fs::remove_file(bak);
            }
        }
        Ok(())
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
}
