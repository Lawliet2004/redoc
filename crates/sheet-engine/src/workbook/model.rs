use crate::cell::{AutoFilterState, MergeRange, SheetCell};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;
use redoc_formula::{Expr, FormulaValue};

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SheetData {
    pub id: String,
    pub name: String,
    pub cells: BTreeMap<String, SheetCell>, // Key format "row:col"
    pub col_widths: BTreeMap<u32, f64>,
    pub row_heights: BTreeMap<u32, f64>,
    pub freeze_rows: u32,
    pub freeze_cols: u32,
    #[serde(default)]
    pub charts: Vec<ChartModel>,
    #[serde(default)]
    pub filter_query: Option<String>,
    #[serde(default)]
    pub merges: Vec<MergeRange>,
    #[serde(default)]
    pub auto_filter: Option<AutoFilterState>,
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
            cells: BTreeMap::new(),
            col_widths: BTreeMap::new(),
            row_heights: BTreeMap::new(),
            freeze_rows: 0,
            freeze_cols: 0,
            charts: Vec::new(),
            filter_query: None,
            merges: Vec::new(),
            auto_filter: None,
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
    pub recalc_plans: Vec<Option<RecalcPlan>>,
    #[serde(skip)]
    #[specta(skip)]
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
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CellRange {
    pub start_row: u32,
    pub end_row: u32,
    pub start_col: u32,
    pub end_col: u32,
}
