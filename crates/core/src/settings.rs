use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub theme: String, // "light" | "dark" | "system"
    pub autosave_interval_ms: u64,
    pub spellcheck_enabled: bool,
    pub font_size_default: u32,
    pub telemetry_enabled: bool,
    #[serde(default = "default_zoom_level")]
    pub zoom_level: u32,
    #[serde(default = "default_check_for_updates")]
    pub check_for_updates: bool,
}

fn default_zoom_level() -> u32 {
    100
}

fn default_check_for_updates() -> bool {
    true
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            theme: "system".to_string(),
            autosave_interval_ms: 2000,
            spellcheck_enabled: true,
            font_size_default: 12,
            telemetry_enabled: false,
            zoom_level: 100,
            check_for_updates: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserializes_without_check_for_updates() {
        let settings: AppSettings =
            serde_json::from_str(r#"{"theme":"dark","autosaveIntervalMs":1000,"spellcheckEnabled":true,"fontSizeDefault":12,"telemetryEnabled":false}"#)
                .expect("settings");
        assert!(settings.check_for_updates);
        assert_eq!(settings.zoom_level, 100);
    }
}
