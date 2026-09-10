use redoc_file_io::container::{AssetInfo, RedocContainer};
use std::io::Write;
use tempfile::NamedTempFile;

#[test]
fn doc_round_trips() {
    let body = serde_json::json!({"type": "doc", "content": []});
    let mut container = RedocContainer::new("doc", "Test", body.clone());
    let tmp = NamedTempFile::new().unwrap();
    container.save_atomic(tmp.path()).unwrap();
    let loaded = RedocContainer::read_from_file(tmp.path()).unwrap();
    assert_eq!(loaded.body, body);
}

#[test]
fn sheet_round_trips() {
    let body = serde_json::json!({
        "sheets": [{
            "name": "Sheet1",
            "cells": {},
            "merges": [],
            "freeze_row": 0,
            "freeze_col": 0
        }]
    });
    let mut container = RedocContainer::new("sheet", "TestSheet", body.clone());
    let tmp = NamedTempFile::new().unwrap();
    container.save_atomic(tmp.path()).unwrap();
    let loaded = RedocContainer::read_from_file(tmp.path()).unwrap();
    assert_eq!(loaded.body, body);
}

#[test]
fn slide_round_trips() {
    let body = serde_json::json!({ "type": "slide", "slides": [] });
    let mut container = RedocContainer::new("slide", "TestSlide", body.clone());
    let tmp = NamedTempFile::new().unwrap();
    container.save_atomic(tmp.path()).unwrap();
    let loaded = RedocContainer::read_from_file(tmp.path()).unwrap();
    assert_eq!(loaded.body, body);
}

#[test]
fn corrupt_zip_returns_error_not_panic() {
    let mut tmp = NamedTempFile::new().unwrap();
    tmp.write_all(b"this is not a zip file at all").unwrap();
    let result = RedocContainer::read_from_file(tmp.path());
    assert!(result.is_err(), "Expected error for corrupt zip, got ok");
}

#[test]
fn missing_asset() {
    let tmp = NamedTempFile::new().unwrap();
    let body = serde_json::json!({ "type": "doc", "content": [] });
    let mut container = RedocContainer::new("doc", "Test Doc", body);

    container.meta.assets.push(AssetInfo {
        hash: "fake_hash".to_string(),
        mime: "image/png".to_string(),
        name: "fake.png".to_string(),
        size: 100,
    });

    container.save_atomic(tmp.path()).unwrap();

    let loaded = RedocContainer::read_from_file(tmp.path()).unwrap();
    assert_eq!(loaded.meta.assets.len(), 1);
    assert!(!loaded.assets_data.contains_key("fake_hash"));
    assert_eq!(loaded.repair.missing_assets, vec!["fake_hash".to_string()]);
    assert!(!loaded.repair.warnings.is_empty());
}
