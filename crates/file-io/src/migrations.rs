use serde_json::Value;

pub fn migrate_version(body: Value, from_version: u32, to_version: u32) -> Result<Value, String> {
    let mut current = from_version;
    let data = body;
    while current < to_version {
        match current {
            0 => {
                // v0 -> v1 migration
                current = 1;
            }
            1 => {
                // v1 -> v2 migration hook if needed in future
                current = 2;
            }
            _ => return Err(format!("No migration path for version {}", current)),
        }
    }
    Ok(data)
}
