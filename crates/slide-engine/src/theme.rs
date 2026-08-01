use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SlideTheme {
    pub id: String,
    pub name: String,
    pub bg_color: String,
    pub text_color: String,
    pub accent_color: String,
    pub font_family: String,
}

impl Default for SlideTheme {
    fn default() -> Self {
        Self {
            id: "default-light".to_string(),
            name: "Modern Light".to_string(),
            bg_color: "#ffffff".to_string(),
            text_color: "#1e293b".to_string(),
            accent_color: "#3b82f6".to_string(),
            font_family: "Inter, sans-serif".to_string(),
        }
    }
}
