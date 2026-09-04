use serde_json::Value;

pub fn migrate_version(
    mut body: Value,
    from_version: u32,
    to_version: u32,
) -> Result<Value, String> {
    let mut current = from_version;
    while current < to_version {
        match current {
            0 => {
                // v0 containers stored body JSON without an explicit content schema marker.
                if let Value::Object(ref mut map) = body {
                    map.entry("contentVersion")
                        .or_insert(Value::Number(1.into()));
                }
                current = 1;
            }
            1 => {
                // v1 -> v2: normalize legacy snake_case page-setup keys and
                // stamp the v2 content marker. Atomic save + sha256 asset
                // verification are unchanged (see container.rs).
                if let Value::Object(ref mut map) = body {
                    if !map.contains_key("pageSetup") {
                        if let Some(legacy) = map.remove("page_setup") {
                            map.insert("pageSetup".to_string(), legacy);
                        }
                    }
                    map.insert("contentVersion".to_string(), Value::Number(2.into()));
                }
                current = 2;
            }
            _ => return Err(format!("No migration path for version {}", current)),
        }
    }
    Ok(body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CURRENT_FORMAT_VERSION;
    use serde_json::json;

    #[test]
    fn migrate_v0_to_current_adds_content_version_marker() {
        let body = json!({ "type": "doc", "content": [] });
        let migrated =
            migrate_version(body, 0, CURRENT_FORMAT_VERSION).expect("migration succeeds");
        assert_eq!(migrated["contentVersion"], 2);
    }

    #[test]
    fn migrate_is_noop_when_already_at_target_version() {
        let body = json!({ "type": "doc", "contentVersion": 1 });
        let migrated =
            migrate_version(body.clone(), CURRENT_FORMAT_VERSION, CURRENT_FORMAT_VERSION)
                .expect("noop migration");
        assert_eq!(migrated, body);
    }

    #[test]
    fn migrate_rejects_unknown_source_version() {
        let body = json!({});
        let err = migrate_version(body, 2, 3).unwrap_err();
        assert!(err.contains("No migration path"));
    }
}
