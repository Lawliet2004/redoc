use crate::cell::{AutoFilterState, MergeRange, SheetCell};
use redoc_formula::{Expr, FormulaValue};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SheetData {
    pub id: String,
    pub name: String,
    /// Sheet tab color (#RRGGBB) for XLSX round-trip and UI recolor.
    #[serde(default)]
    pub tab_color: Option<String>,
    pub cells: BTreeMap<String, SheetCell>, // Key format "row:col"
    pub col_widths: BTreeMap<u32, f64>,
    pub row_heights: BTreeMap<u32, f64>,
    pub freeze_rows: u32,
    pub freeze_cols: u32,
    /// Hidden column indices (1-based); skipped by rendering and scroll math.
    #[serde(default)]
    pub hidden_cols: std::collections::BTreeSet<u32>,
    /// Hidden row indices (1-based); skipped by rendering and scroll math.
    #[serde(default)]
    pub hidden_rows: std::collections::BTreeSet<u32>,
    #[serde(default)]
    pub charts: Vec<ChartModel>,
    #[serde(default)]
    pub filter_query: Option<String>,
    #[serde(default)]
    pub merges: Vec<MergeRange>,
    #[serde(default)]
    pub auto_filter: Option<AutoFilterState>,
    #[serde(default)]
    pub conditional_formatting: Vec<ConditionalFormattingRule>,
    #[serde(default)]
    pub pivot_tables: Vec<PivotTableModel>,
    #[serde(default)]
    pub tables: Vec<TableModel>,
    #[serde(default)]
    pub scenarios: Vec<ScenarioModel>,
    #[serde(default)]
    pub slicers: Vec<SlicerModel>,
    /// Bounded materialized results for dynamic-array formulas. The origin
    /// remains the only editable formula cell; values are render metadata.
    #[serde(default)]
    pub spills: Vec<ArraySpill>,
    /// Cell-anchored review comments (offline collaboration affordance).
    #[serde(default)]
    pub comments: Vec<CellCommentModel>,
}

pub const MAX_ARRAY_SPILL_CELLS: usize = 100_000;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ArraySpill {
    pub origin_row: u32,
    pub origin_col: u32,
    pub rows: u32,
    pub cols: u32,
    pub values: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ConditionalFormattingRule {
    pub range: ConditionalFormattingRange,
    #[serde(rename = "type")]
    pub rule_type: String,
    #[serde(default)]
    pub value: Option<String>,
    #[serde(default)]
    pub value2: Option<String>,
    #[serde(default)]
    pub style: Option<ConditionalFormattingStyle>,
    #[serde(default)]
    pub scale_colors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ConditionalFormattingRange {
    pub start_row: u32,
    pub end_row: u32,
    pub start_col: u32,
    pub end_col: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ConditionalFormattingStyle {
    #[serde(default)]
    pub font_color: Option<String>,
    #[serde(default)]
    pub bg_color: Option<String>,
    #[serde(default)]
    pub bold: Option<bool>,
    #[serde(default)]
    pub italic: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PivotTableModel {
    pub id: String,
    pub source_range: CellRange,
    pub row_field: u32,
    /// When set, the pivot crosses rows × this column field (multi-field).
    #[serde(default)]
    pub column_field: Option<u32>,
    pub value_field: u32,
    pub aggregation: String,
    pub output_start_row: u32,
    pub output_start_col: u32,
    #[serde(default)]
    pub output_row_count: Option<u32>,
    #[serde(default)]
    pub output_col_count: Option<u32>,
}

fn default_table_header_row() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TableModel {
    pub id: String,
    pub name: String,
    pub range: CellRange,
    #[serde(default)]
    pub columns: Vec<String>,
    #[serde(default)]
    pub style: Option<String>,
    #[serde(default = "default_table_header_row")]
    pub show_header_row: bool,
    #[serde(default)]
    pub show_total_row: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ScenarioCellChange {
    pub row: u32,
    pub col: u32,
    pub raw_value: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ScenarioModel {
    pub id: String,
    pub name: String,
    pub changes: Vec<ScenarioCellChange>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SlicerModel {
    pub id: String,
    pub title: String,
    pub source_range: CellRange,
    pub column: u32,
    #[serde(default)]
    pub selected_values: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CellCommentModel {
    pub id: String,
    pub row: u32,
    pub col: u32,
    pub author: String,
    pub text: String,
    #[serde(default)]
    pub resolved: bool,
    #[serde(default)]
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ChartModel {
    pub chart_type: String,
    #[serde(default)]
    pub title: Option<String>,
    pub start_row: u32,
    pub end_row: u32,
    pub start_col: u32,
    pub end_col: u32,
}

impl SheetData {
    pub fn new(id: &str, name: &str) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            tab_color: None,
            cells: BTreeMap::new(),
            col_widths: BTreeMap::new(),
            row_heights: BTreeMap::new(),
            freeze_rows: 0,
            freeze_cols: 0,
            hidden_cols: std::collections::BTreeSet::new(),
            hidden_rows: std::collections::BTreeSet::new(),
            charts: Vec::new(),
            filter_query: None,
            merges: Vec::new(),
            auto_filter: None,
            conditional_formatting: Vec::new(),
            pivot_tables: Vec::new(),
            tables: Vec::new(),
            scenarios: Vec::new(),
            slicers: Vec::new(),
            spills: Vec::new(),
            comments: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct NamedRange {
    pub name: String,
    pub range_str: String,
    pub sheet: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct WorkbookModel {
    pub sheets: Vec<SheetData>,
    pub active_sheet_index: usize,
    #[serde(default)]
    pub named_ranges: Vec<NamedRange>,
    #[serde(skip)]
    #[specta(skip)]
    pub formula_cache: HashMap<(usize, String), (String, Arc<Expr>)>,
    #[serde(skip)]
    #[specta(skip)]
    pub cell_value_cache: Vec<HashMap<(u32, u32), FormulaValue>>,
    #[serde(skip)]
    #[specta(skip)]
    pub cell_value_cache_valid: Vec<bool>,
    #[serde(skip)]
    #[specta(skip)]
    pub formula_values_valid: Vec<bool>,
    #[serde(skip)]
    #[specta(skip)]
    pub recalc_plans: Vec<Option<RecalcPlan>>,
    #[serde(skip)]
    #[specta(skip)]
    #[allow(clippy::type_complexity)]
    pub cross_sheet_dependents: HashMap<(usize, u32, u32), HashSet<(usize, u32, u32)>>,
    #[serde(skip)]
    #[specta(skip)]
    pub cross_sheet_index_valid: bool,
}

#[derive(Debug, Clone)]
pub struct RecalcPlan {
    pub(crate) formulas: HashMap<(u32, u32), Arc<Expr>>,
    pub(crate) order: Option<Vec<(u32, u32)>>,
    pub(crate) dependents: HashMap<(u32, u32), HashSet<(u32, u32)>>,
    /// Cells trapped inside (or downstream of) a dependency cycle. Only these
    /// evaluate to #CYCLE!; the rest of the sheet recalculates normally.
    pub(crate) poisoned: Vec<(u32, u32)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CellRange {
    pub start_row: u32,
    pub end_row: u32,
    pub start_col: u32,
    pub end_col: u32,
}
