use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub enum ElementKind {
    #[serde(rename = "Text", alias = "text", rename_all = "camelCase")]
    Text {
        text: String,
        font_size: f64,
        font_family: String,
        color: String,
        align: String,
        #[serde(default)]
        bold: bool,
        #[serde(default)]
        italic: bool,
        #[serde(default)]
        underline: bool,
        #[serde(default)]
        bullets: bool,
    },
    #[serde(rename = "Shape", alias = "shape", rename_all = "camelCase")]
    Shape {
        shape_type: String,
        fill_color: String,
        stroke_color: String,
        stroke_width: f64,
    }, // "rect" | "ellipse" | "line" | "arrow"
    #[serde(rename = "Image", alias = "image", rename_all = "camelCase")]
    Image { asset_hash: String, mime: String },
    #[serde(rename = "Table", alias = "table", rename_all = "camelCase")]
    Table {
        rows: usize,
        cols: usize,
        data: Vec<Vec<String>>,
    },
    #[serde(rename = "Chart", alias = "chart", rename_all = "camelCase")]
    Chart {
        chart_type: String,
        data: Vec<f64>,
        labels: Vec<String>,
    },
}

fn default_entrance() -> String {
    "none".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SlideElement {
    pub id: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub rotation: f64,
    pub z_index: i32,
    /// Per-element entrance animation: "none" | "fade"
    #[serde(default = "default_entrance")]
    pub entrance: String,
    pub kind: ElementKind,
}
