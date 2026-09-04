use serde::{Deserialize, Serialize};
use specta::Type;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CellStyle {
    pub bold: Option<bool>,
    pub italic: Option<bool>,
    pub underline: Option<bool>,
    pub font_color: Option<String>,
    pub bg_color: Option<String>,
    pub align: Option<String>,  // "left" | "center" | "right"
    pub format: Option<String>, // "general" | "currency" | "percent" | "number"
    #[serde(default)]
    pub wrap: Option<bool>,
    #[serde(default)]
    pub v_align: Option<String>, // "top" | "middle" | "bottom"
    #[serde(default)]
    pub validation: Option<ListValidation>,
    #[serde(default)]
    pub hyperlink: Option<String>,
    /// Optional bounded inline image associated with this cell in Redoc.
    /// PNG/JPEG values can round-trip through native XLSX drawings.
    #[serde(default)]
    pub image: Option<String>,
    /// Font family name; survives .redoc saves and XLSX export. Older files
    /// deserialize without it (serde default).
    #[serde(default)]
    pub font_family: Option<String>,
    /// Font size in points.
    #[serde(default)]
    pub font_size: Option<f64>,
    /// Digits after the decimal point for numeric display (0-10).
    #[serde(default)]
    pub decimals: Option<u32>,
    /// Per-side cell borders; round-trips through native XLSX border styles.
    #[serde(default)]
    pub borders: Option<CellBorders>,
}

/// Per-side border definition shared by the editor, model, and XLSX round-trip.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CellBorders {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top: Option<BorderEdge>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub right: Option<BorderEdge>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bottom: Option<BorderEdge>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub left: Option<BorderEdge>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct BorderEdge {
    /// Bounded Excel-compatible border line styles.
    pub style: String, // "thin" | "medium" | "thick" | "dashed" | "dotted" | "double"
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ListValidation {
    #[serde(rename = "type")]
    pub validation_type: String,
    pub options: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub formula: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SheetCell {
    pub raw_value: String,       // E.g. "123" or "=SUM(A1:A5)"
    pub display_value: String,   // Computed string for display
    pub formula: Option<String>, // Raw formula string if starts with '='
    pub style: Option<CellStyle>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MergeRange {
    pub start_row: u32,
    pub end_row: u32,
    pub start_col: u32,
    pub end_col: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, Default)]
#[serde(rename_all = "camelCase")]
pub struct AutoFilterState {
    pub enabled: bool,
    pub start_row: u32,
    pub end_row: u32,
    pub start_col: u32,
    pub end_col: u32,
    /// Per-column selected values (empty = show all). Key = column index.
    pub column_filters: BTreeMap<u32, Vec<String>>,
}
