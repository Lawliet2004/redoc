// SPDX-License-Identifier: MIT OR Apache-2.0
use redoc_core::recovery::RecoveryManager;
use redoc_file_io::{RedocContainer, SnapshotManager};
use serde_json::json;
use std::time::{SystemTime, UNIX_EPOCH};

fn unique_dir() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
        .to_string()
}

fn temp_recovery_dir(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("redoc-crash-{}-{}", tag, unique_dir()));
    std::fs::create_dir_all(&dir).expect("create recovery dir");
    dir
}

fn write_valid_snapshot(dir: &std::path::Path, doc_id: &str, title: &str) {
    let manager = SnapshotManager::new(dir.join("autosave"));
    let mut container = RedocContainer::new("doc", title, json!({ "type": "doc", "content": [] }));
    container.meta.id = doc_id.to_string();
    manager
        .write_snapshot(doc_id, &mut container)
        .expect("write snapshot");
}

#[test]
fn snapshot_and_sentinel_are_listed_for_recovery() {
    let dir = temp_recovery_dir("listed");
    write_valid_snapshot(&dir, "doc-1", "Untitled crash doc");
    std::fs::write(dir.join("running.sentinel"), b"PID: 1").expect("write sentinel");

    let manager = RecoveryManager::new(dir.clone());
    assert!(manager.had_unclean_shutdown());
    let docs = manager.check_recovery_needed();
    assert_eq!(docs.len(), 1);
    assert_eq!(docs[0].id, "doc-1");
    assert_eq!(docs[0].title, "Untitled crash doc");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn corrupt_snapshot_is_skipped_without_panic() {
    let dir = temp_recovery_dir("corrupt");
    let autosave = dir.join("autosave");
    std::fs::create_dir_all(&autosave).expect("create autosave dir");
    std::fs::write(autosave.join("good.redoc"), b"not a zip at all").expect("write junk");
    std::fs::write(dir.join("running.sentinel"), b"PID: 1").expect("write sentinel");

    let manager = RecoveryManager::new(dir.clone());
    let docs = manager.check_recovery_needed();
    assert!(
        docs.is_empty(),
        "corrupt snapshots must be skipped, listed: {:?}",
        docs
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn truncated_snapshot_missing_meta_is_skipped() {
    let dir = temp_recovery_dir("truncated");
    let autosave = dir.join("autosave");
    std::fs::create_dir_all(&autosave).expect("create autosave dir");
    let mut truncated =
        RedocContainer::new("doc", "Truncated", json!({ "type": "doc", "content": [] }));
    truncated
        .save_atomic(autosave.join("trunc.redoc"))
        .expect("seed zip");
    let bytes = std::fs::read(autosave.join("trunc.redoc")).expect("read zip");
    std::fs::write(autosave.join("trunc.redoc"), &bytes[..bytes.len() / 2]).expect("truncate");
    std::fs::write(dir.join("running.sentinel"), b"PID: 1").expect("write sentinel");

    let manager = RecoveryManager::new(dir.clone());
    let docs = manager.check_recovery_needed();
    assert!(docs.is_empty(), "truncated snapshot must not be listed");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn rotation_survives_partially_written_bak() {
    let dir = temp_recovery_dir("rotation");
    let autosave = dir.join("autosave");
    std::fs::create_dir_all(&autosave).expect("create autosave dir");
    let manager = SnapshotManager::new(autosave.clone());
    let doc_id = "doc-rot";

    let mut v1 = RedocContainer::new("doc", "V1", json!({ "v": 1 }));
    manager.write_snapshot(doc_id, &mut v1).expect("write 1");
    let mut v2 = RedocContainer::new("doc", "V2", json!({ "v": 2 }));
    manager.write_snapshot(doc_id, &mut v2).expect("write 2");
    let mut v3 = RedocContainer::new("doc", "V3", json!({ "v": 3 }));
    manager.write_snapshot(doc_id, &mut v3).expect("write 3");

    let primary = autosave.join(format!("{}.redoc", doc_id));
    let bak1 = autosave.join(format!("{}.redoc.bak1", doc_id));
    let bak2 = autosave.join(format!("{}.redoc.bak2", doc_id));

    std::fs::write(&bak2, b"partially written bak").expect("corrupt bak2");

    let mut v4 = RedocContainer::new("doc", "V4", json!({ "v": 4 }));
    manager
        .write_snapshot(doc_id, &mut v4)
        .expect("rotation continues past corrupt bak");

    assert_eq!(
        RedocContainer::read_from_file(&primary)
            .expect("read primary")
            .body,
        json!({ "v": 4 })
    );
    assert_eq!(
        RedocContainer::read_from_file(&bak1)
            .expect("read bak1")
            .body,
        json!({ "v": 3 })
    );
    let bak2_container = RedocContainer::read_from_file(&bak2).expect("bak2 readable");
    assert_eq!(bak2_container.body, json!({ "v": 2 }));

    let history = manager.list_history(doc_id);
    assert!(
        history.iter().any(|entry| entry.slot == "current"
            && entry.body_hash
                != history
                    .iter()
                    .find(|e| e.slot == "bak1")
                    .expect("bak1 in history")
                    .body_hash),
        "history must list distinct current and bak1 slots"
    );
    let _ = std::fs::remove_dir_all(dir);
}
