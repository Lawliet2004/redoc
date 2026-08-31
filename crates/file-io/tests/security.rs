use redoc_file_io::{FileIoError, RedocContainer};
use std::io::Write;
use tempfile::NamedTempFile;
use zip::write::FileOptions;

fn write_archive(entries: &[(&str, &[u8])]) -> NamedTempFile {
    let mut file = NamedTempFile::new().expect("temporary archive");
    {
        let mut zip = zip::ZipWriter::new(&mut file);
        let options = FileOptions::default();
        for (name, bytes) in entries {
            zip.start_file(*name, options).expect("start entry");
            zip.write_all(bytes).expect("write entry");
        }
        zip.finish().expect("finish archive");
    }
    file
}

fn minimal_meta(asset_json: &str) -> Vec<u8> {
    format!(
        r#"{{"formatVersion":1,"id":"doc","mode":"doc","title":"Test","createdAt":0,"updatedAt":0,"appVersion":"0.1.0","assets":[{}]}}"#,
        asset_json
    )
    .into_bytes()
}

#[test]
fn rejects_oversized_body_before_parsing() {
    let oversized = vec![b' '; 16 * 1024 * 1024 + 1];
    let archive = write_archive(&[("meta.json", &minimal_meta("")), ("body.json", &oversized)]);

    let error = RedocContainer::read_from_file(archive.path()).expect_err("oversized body must fail");
    assert!(matches!(error, FileIoError::EntryTooLarge { name, .. } if name == "body.json"));
}

#[test]
fn rejects_asset_hash_and_size_mismatches() {
    let declared_hash = "0000000000000000000000000000000000000000000000000000000000000000";
    let meta = minimal_meta(&format!(
        r#"{{"hash":"{}","mime":"image/png","name":"image.png","size":3}}"#,
        declared_hash
    ));
    let archive = write_archive(&[
        ("meta.json", &meta),
        ("body.json", br#"{"type":"doc","content":[]}"#),
        ("assets/0000000000000000000000000000000000000000000000000000000000000000", b"abc"),
    ]);

    let error = RedocContainer::read_from_file(archive.path()).expect_err("hash mismatch must fail");
    assert!(matches!(error, FileIoError::AssetHashMismatch { .. }));
}

#[test]
fn rejects_invalid_asset_path_reference() {
    let meta = minimal_meta(r#"{"hash":"../escape","mime":"image/png","name":"image.png","size":1}"#);
    let archive = write_archive(&[
        ("meta.json", &meta),
        ("body.json", br#"{"type":"doc","content":[]}"#),
    ]);

    let error = RedocContainer::read_from_file(archive.path()).expect_err("invalid asset must fail");
    assert!(matches!(error, FileIoError::InvalidAsset(value) if value == "../escape"));
}
