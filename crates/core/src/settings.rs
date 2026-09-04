// SPDX-License-Identifier: MIT OR Apache-2.0
// Local-first author profile: single-device identity used to attribute
// comments, revisions, and audit events. No server account is required.
use serde::{Deserialize, Serialize};
use specta::Type;

/// Local-first author identity stored in app settings.
///
/// Replaces ad-hoc `author: None` / hardcoded `"You"` / `"Owner: me"`
/// fallbacks: editors resolve the display name from here and only fall
/// back to `"You"` for older documents that predate the profile.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AuthorProfile {
    pub display_name: String,
    pub email: Option<String>,
    #[serde(default)]
    pub color: Option<String>,
}

fn default_author() -> AuthorProfile {
    AuthorProfile {
        display_name: "You".to_string(),
        email: None,
        color: None,
    }
}

/// Normalize a display name to a bounded, non-empty value.
pub fn sanitize_author_name(value: &str) -> String {
    let trimmed = value.trim().replace(|c: char| c.is_control(), "");
    let trimmed = trimmed.trim();
    if trimmed.is_empty() {
        return "You".to_string();
    }
    // Bound to 80 chars to match comment-author bounds.
    trimmed.chars().take(80).collect::<String>()
}

impl Default for AuthorProfile {
    fn default() -> Self {
        default_author()
    }
}

impl AuthorProfile {
    pub fn normalized(mut self) -> Self {
        self.display_name = sanitize_author_name(&self.display_name);
        if let Some(email) = self.email.take() {
            let email = email.trim().chars().take(254).collect::<String>();
            self.email = if email.is_empty() { None } else { Some(email) };
        }
        if let Some(color) = self.color.take() {
            let color = color.trim().chars().take(32).collect::<String>();
            self.color = if color.is_empty() { None } else { Some(color) };
        }
        self
    }
}

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
    #[serde(default = "default_author")]
    pub author: AuthorProfile,
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
            author: default_author(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_to_you_author_profile() {
        let settings = AppSettings::default();
        assert_eq!(settings.author.display_name, "You");
        assert!(settings.author.email.is_none());
    }

    #[test]
    fn sanitizes_author_names_to_bounded_fallback() {
        assert_eq!(sanitize_author_name("   "), "You");
        assert_eq!(sanitize_author_name("  Ada  "), "Ada");
        assert_eq!(sanitize_author_name("a\u{0}b"), "ab");
        assert_eq!(sanitize_author_name(&"x".repeat(200)).len(), 80);
    }

    #[test]
    fn deserializes_legacy_settings_without_author() {
        let settings: AppSettings =
            serde_json::from_str(r#"{"theme":"dark","autosaveIntervalMs":1000,"spellcheckEnabled":true,"fontSizeDefault":12,"telemetryEnabled":false}"#)
                .expect("settings");
        assert!(settings.check_for_updates);
        assert_eq!(settings.zoom_level, 100);
    }
}
