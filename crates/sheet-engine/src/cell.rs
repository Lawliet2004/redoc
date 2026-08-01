use serde::{Deserialize, Serialize};
use specta::Type;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
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
