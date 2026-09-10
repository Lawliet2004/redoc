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
        #[serde(default)]
        text: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        fill_gradient: Option<ShapeGradient>,
        #[serde(default)]
        shadow: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        font_family: Option<String>,
        #[serde(default)]
        bold: bool,
        #[serde(default)]
        italic: bool,
        #[serde(default)]
        underline: bool,
    }, // "rect" | "ellipse" | "line" | "arrow"
    #[serde(rename = "Image", alias = "image", rename_all = "camelCase")]
    Image { asset_hash: String, mime: String },
    #[serde(rename = "Table", alias = "table", rename_all = "camelCase")]
    Table {
        rows: usize,
        cols: usize,
        data: Vec<Vec<String>>,
        /// Merged ranges anchored at (row, col) with row/col spans.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        merges: Vec<TableMerge>,
        #[serde(default)]
        header_row: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        table_style: Option<String>,
    },
    #[serde(rename = "Chart", alias = "chart", rename_all = "camelCase")]
    Chart {
        chart_type: String,
        data: Vec<f64>,
        labels: Vec<String>,
        #[serde(default = "default_true")]
        legend: bool,
        #[serde(default = "default_true")]
        show_labels: bool,
        #[serde(default = "default_true")]
        show_axes: bool,
    },
}

fn default_true() -> bool {
    true
}

/// Bounded linear-gradient fill for closed shapes.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ShapeGradient {
    pub from: String,
    pub to: String,
    #[serde(default = "default_gradient_angle")]
    pub angle: u16,
}

fn default_gradient_angle() -> u16 {
    90
}

/// Merged table range anchored at (row, col).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TableMerge {
    pub r: usize,
    pub c: usize,
    pub rowspan: usize,
    pub colspan: usize,
}

fn default_entrance() -> String {
    "none".to_string()
}

fn default_exit() -> String {
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
    /// Per-element entrance animation: "none" | "fade" | "zoom"
    #[serde(default = "default_entrance")]
    pub entrance: String,
    /// Optional timing metadata for the supported entrance-animation subset.
    /// Keeping these fields optional preserves older `.redoc` documents and
    /// avoids serializing defaults when no animation is configured.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entrance_delay_ms: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entrance_duration_ms: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entrance_order: Option<u32>,
    /// Per-element exit animation: "none" | "fade". Played when the slide is
    /// left in presenter view; persisted alongside the entrance metadata.
    #[serde(default = "default_exit")]
    pub exit: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_duration_ms: Option<u32>,
    /// Optional safe hyperlink target attached to the whole element.
    ///
    /// The editor accepts bounded http(s), mailto, and tel targets. PPTX
    /// export emits these as external relationship-backed hyperlinks; older
    /// Redoc documents omit this field and continue to deserialize cleanly.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hyperlink: Option<String>,
    pub kind: ElementKind,
}
