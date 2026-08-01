use crate::cell::{AutoFilterState, MergeRange, SheetCell};
use redoc_formula::FormulaError;
use redoc_formula::{eval_expr, parse_formula, CellProvider, Expr, FormulaValue};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;

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
    formulas: HashMap<(u32, u32), Arc<Expr>>,
    order: Option<Vec<(u32, u32)>>,
    dependents: HashMap<(u32, u32), HashSet<(u32, u32)>>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CellRange {
    pub start_row: u32,
    pub end_row: u32,
    pub start_col: u32,
    pub end_col: u32,
}

impl WorkbookModel {
    pub fn new_default() -> Self {
        Self {
            sheets: vec![SheetData::new("sheet-1", "Sheet1")],
            active_sheet_index: 0,
            named_ranges: Vec::new(),
            formula_cache: HashMap::new(),
            cell_value_cache: vec![HashMap::new()],
            cell_value_cache_valid: vec![false],
            recalc_plans: vec![None],
            cross_sheet_dependents: HashMap::new(),
            cross_sheet_index_valid: false,
        }
    }

    pub fn set_cell_value(&mut self, sheet_idx: usize, row: u32, col: u32, raw_value: String) {
        if let Some(sheet) = self.sheets.get_mut(sheet_idx) {
            let key = format!("{}:{}", row, col);
            if raw_value.trim().is_empty() {
                sheet.cells.remove(&key);
            } else {
                let existing_style = sheet.cells.get(&key).and_then(|c| c.style.clone());
                let formula = if raw_value.starts_with('=') {
                    Some(raw_value.clone())
                } else {
                    None
                };
                let display_value = if formula.is_some() {
                    "#EVAL...".to_string()
                } else {
                    raw_value.clone()
                };

                let cell = SheetCell {
                    raw_value,
                    display_value,
                    formula,
                    style: existing_style,
                };
                sheet.cells.insert(key, cell);
            }
        }
        self.invalidate_cell_value_cache(sheet_idx);
        self.recalculate_dirty(sheet_idx, row, col);
    }

    pub fn recalculate(&mut self, sheet_idx: usize) {
        let _ = self.recalculate_internal(sheet_idx, None);
    }

    pub fn recalculate_dirty(&mut self, sheet_idx: usize, row: u32, col: u32) {
        self.ensure_cross_sheet_index();
        let mut queue = vec![(sheet_idx, row, col)];
        let mut seen = HashSet::new();
        while let Some((s, r, c)) = queue.pop() {
            if !seen.insert((s, r, c)) {
                continue;
            }
            let recalculated = self.recalculate_internal(s, Some((r, c)));
            if let Some(deps) = self.cross_sheet_dependents.get(&(s, r, c)) {
                for dep in deps {
                    queue.push(*dep);
                }
            }
            for coord in recalculated {
                if let Some(deps) = self.cross_sheet_dependents.get(&(s, coord.0, coord.1)) {
                    for dep in deps {
                        queue.push(*dep);
                    }
                }
            }
        }
    }

    fn recalculate_internal(
        &mut self,
        sheet_idx: usize,
        dirty_cell: Option<(u32, u32)>,
    ) -> HashSet<(u32, u32)> {
        if sheet_idx >= self.sheets.len() {
            return HashSet::new();
        }
        self.ensure_cell_value_cache();
        self.ensure_recalc_plan(sheet_idx);
        let Some(plan) = self.recalc_plans.get(sheet_idx).and_then(Option::as_ref) else {
            return HashSet::new();
        };
        let Some(order) = plan.order.clone() else {
            let formula_coords: Vec<_> = plan.formulas.keys().copied().collect();
            for coord in &formula_coords {
                self.cell_value_cache[sheet_idx]
                    .insert(*coord, FormulaValue::Error(FormulaError::Cycle));
            }
            if let Some(sheet) = self.sheets.get_mut(sheet_idx) {
                for coord in formula_coords {
                    if let Some(cell) = sheet.cells.get_mut(&format!("{}:{}", coord.0, coord.1)) {
                        cell.display_value = FormulaError::Cycle.to_str().to_string();
                    }
                }
            }
            return HashSet::new();
        };
        let affected = dirty_cell.map(|start| {
            let mut affected = HashSet::from([start]);
            let mut pending = vec![start];
            while let Some(cell) = pending.pop() {
                if let Some(dependents) = plan.dependents.get(&cell) {
                    for dependent in dependents {
                        if affected.insert(*dependent) {
                            pending.push(*dependent);
                        }
                    }
                }
            }
            affected
        });

        let sheet_names: Vec<String> = self.sheets.iter().map(|sheet| sheet.name.clone()).collect();
        let mut recalculated = HashSet::new();
        for coord in order {
            if affected
                .as_ref()
                .is_some_and(|cells| !cells.contains(&coord))
            {
                continue;
            }
            let Some(ast) = self
                .recalc_plans
                .get(sheet_idx)
                .and_then(Option::as_ref)
                .and_then(|plan| plan.formulas.get(&coord))
                .cloned()
            else {
                continue;
            };
            let value = {
                let provider = WorkbookCellProvider {
                    sheet_names: &sheet_names,
                    values: &self.cell_value_cache,
                    sheet_idx,
                };
                eval_expr(&ast, &provider)
            };
            let display = format_formula_value(&value);
            self.cell_value_cache[sheet_idx].insert(coord, value);
            if let Some(sheet) = self.sheets.get_mut(sheet_idx) {
                if let Some(cell) = sheet.cells.get_mut(&format!("{}:{}", coord.0, coord.1)) {
                    cell.display_value = display;
                }
            }
            recalculated.insert(coord);
        }
        recalculated
    }

    fn ensure_cross_sheet_index(&mut self) {
        if self.cross_sheet_index_valid {
            return;
        }
        self.cross_sheet_dependents.clear();
        let sheet_names: Vec<String> = self.sheets.iter().map(|s| s.name.clone()).collect();
        for (sheet_idx, sheet) in self.sheets.iter().enumerate() {
            for (key, cell) in &sheet.cells {
                let Ok(formula_coord) = parse_key(key) else {
                    continue;
                };
                let Some(ref raw_formula) = cell.formula else {
                    continue;
                };
                let expanded_formula = expand_named_ranges(raw_formula, &self.named_ranges);
                let Ok(ast) = parse_formula(&expanded_formula) else {
                    continue;
                };
                let external = extract_external_dependencies(&ast, sheet_idx, &sheet_names);
                for (src_sheet, src_row, src_col) in external {
                    self.cross_sheet_dependents
                        .entry((src_sheet, src_row, src_col))
                        .or_default()
                        .insert((sheet_idx, formula_coord.0, formula_coord.1));
                }
            }
        }
        self.cross_sheet_index_valid = true;
    }

    fn ensure_recalc_plan(&mut self, sheet_idx: usize) {
        if self.recalc_plans.len() != self.sheets.len() {
            self.recalc_plans.resize(self.sheets.len(), None);
        }
        if self.recalc_plans[sheet_idx].is_some() {
            return;
        }
        let mut formulas: HashMap<(u32, u32), Arc<Expr>> = HashMap::new();
        let mut all_cells = Vec::new();
        let mut formula_inputs = Vec::new();
        if let Some(sheet) = self.sheets.get(sheet_idx) {
            for (key, cell) in &sheet.cells {
                let Ok(coord) = parse_key(key) else {
                    continue;
                };
                all_cells.push(coord);
                if let Some(ref f) = cell.formula {
                    formula_inputs.push((coord, key.clone(), f.clone()));
                }
            }
        }
        for (coord, key, raw_formula) in formula_inputs {
            let cache_key = (sheet_idx, key);
            let expanded_formula = expand_named_ranges(&raw_formula, &self.named_ranges);
            let ast = self
                .formula_cache
                .get(&cache_key)
                .filter(|(cached_formula, _)| cached_formula == &expanded_formula)
                .map(|(_, ast)| Arc::clone(ast))
                .or_else(|| parse_formula(&expanded_formula).ok().map(Arc::new));
            if let Some(ast) = ast {
                self.formula_cache
                    .insert(cache_key, (expanded_formula, Arc::clone(&ast)));
                formulas.insert(coord, ast);
            }
        }

        let formula_coords: Vec<_> = formulas.keys().copied().collect();
        let mut dependency_graph = redoc_formula::DependencyGraph::default();
        for (&coord, ast) in &formulas {
            dependency_graph.update_cell_deps(coord, extract_local_dependencies(ast));
        }

        let order = match dependency_graph.get_recalc_order(&all_cells) {
            Ok(ord) => {
                if ord.is_empty() {
                    Some(formula_coords)
                } else {
                    Some(ord)
                }
            }
            Err(_) => None,
        };
        let dependents = dependency_graph.dependents;

        self.recalc_plans[sheet_idx] = Some(RecalcPlan {
            formulas,
            order,
            dependents,
        });
    }

    fn ensure_cell_value_cache(&mut self) {
        if self.cell_value_cache.len() != self.sheets.len() {
            self.cell_value_cache
                .resize_with(self.sheets.len(), HashMap::new);
            self.cell_value_cache_valid.resize(self.sheets.len(), false);
        }
        for (index, sheet) in self.sheets.iter().enumerate() {
            if self.cell_value_cache_valid[index] {
                continue;
            }
            let mut values = HashMap::with_capacity(sheet.cells.len());
            for (key, cell) in &sheet.cells {
                if let Ok(coord) = parse_key(key) {
                    values.insert(coord, cell_to_formula_value(cell));
                }
            }
            self.cell_value_cache[index] = values;
            self.cell_value_cache_valid[index] = true;
        }
    }

    fn invalidate_cell_value_cache(&mut self, sheet_idx: usize) {
        if self.cell_value_cache.len() != self.sheets.len() {
            self.cell_value_cache
                .resize_with(self.sheets.len(), HashMap::new);
            self.cell_value_cache_valid.resize(self.sheets.len(), false);
        }
        if self.recalc_plans.len() != self.sheets.len() {
            self.recalc_plans.resize(self.sheets.len(), None);
        }
        if let Some(valid) = self.cell_value_cache_valid.get_mut(sheet_idx) {
            *valid = false;
        }
        self.recalc_plans[sheet_idx] = None;
        self.cross_sheet_index_valid = false;
    }

    pub fn fill_series(
        &mut self,
        sheet_idx: usize,
        source_row: u32,
        source_col: u32,
        target_row: u32,
        target_col: u32,
    ) {
        let Some(sheet) = self.sheets.get_mut(sheet_idx) else {
            return;
        };
        let Some(source) = sheet
            .cells
            .get(&format!("{}:{}", source_row, source_col))
            .cloned()
        else {
            return;
        };
        let start_row = source_row.min(target_row);
        let end_row = source_row.max(target_row);
        let start_col = source_col.min(target_col);
        let end_col = source_col.max(target_col);
        let numeric_start = source.raw_value.parse::<f64>().ok();
        for row in start_row..=end_row {
            for col in start_col..=end_col {
                if row == source_row && col == source_col {
                    continue;
                }
                let mut cell = source.clone();
                if let Some(ref form) = source.formula {
                    let delta_row = row as i32 - source_row as i32;
                    let delta_col = col as i32 - source_col as i32;
                    let adjusted = adjust_formula_references(form, delta_row, delta_col);
                    cell.formula = Some(adjusted.clone());
                    cell.raw_value = adjusted;
                    cell.display_value = "#EVAL...".to_string();
                } else if let Some(number) = numeric_start {
                    let distance = (row as i64 - source_row as i64).unsigned_abs()
                        + (col as i64 - source_col as i64).unsigned_abs();
                    let value = number + distance as f64;
                    cell.raw_value = value.to_string();
                    cell.display_value = cell.raw_value.clone();
                    cell.formula = None;
                }
                sheet.cells.insert(format!("{}:{}", row, col), cell);
            }
        }
        self.invalidate_cell_value_cache(sheet_idx);
        self.recalculate(sheet_idx);
    }

    pub fn sort_range(
        &mut self,
        sheet_idx: usize,
        range: CellRange,
        sort_col: u32,
        ascending: bool,
    ) {
        self.sort_range_multi(sheet_idx, range, &[(sort_col, ascending)]);
    }

    /// Stable multi-key sort: `keys` are (column, ascending) in priority order (first key primary).
    pub fn sort_range_multi(&mut self, sheet_idx: usize, range: CellRange, keys: &[(u32, bool)]) {
        if keys.is_empty() {
            return;
        }
        let Some(sheet) = self.sheets.get_mut(sheet_idx) else {
            return;
        };
        let mut rows: Vec<(u32, BTreeMap<u32, SheetCell>)> = (range.start_row..=range.end_row)
            .map(|row| {
                let values = (range.start_col..=range.end_col)
                    .filter_map(|col| {
                        sheet
                            .cells
                            .get(&format!("{}:{}", row, col))
                            .cloned()
                            .map(|cell| (col, cell))
                    })
                    .collect();
                (row, values)
            })
            .collect();
        rows.sort_by(|(_, left), (_, right)| {
            for &(sort_col, ascending) in keys {
                let left_value = left
                    .get(&sort_col)
                    .map(|cell| cell.display_value.as_str())
                    .unwrap_or("");
                let right_value = right
                    .get(&sort_col)
                    .map(|cell| cell.display_value.as_str())
                    .unwrap_or("");
                let left_num = left_value.parse::<f64>().ok();
                let right_num = right_value.parse::<f64>().ok();
                let ordering = match (left_num, right_num) {
                    (Some(a), Some(b)) => a.partial_cmp(&b).unwrap_or(std::cmp::Ordering::Equal),
                    _ => left_value.cmp(right_value),
                };
                let ordering = if ascending {
                    ordering
                } else {
                    ordering.reverse()
                };
                if ordering != std::cmp::Ordering::Equal {
                    return ordering;
                }
            }
            std::cmp::Ordering::Equal
        });
        for row in range.start_row..=range.end_row {
            for col in range.start_col..=range.end_col {
                sheet.cells.remove(&format!("{}:{}", row, col));
            }
        }
        for (offset, (_, values)) in rows.into_iter().enumerate() {
            let row = range.start_row + offset as u32;
            for (col, cell) in values {
                sheet.cells.insert(format!("{}:{}", row, col), cell);
            }
        }
        self.invalidate_cell_value_cache(sheet_idx);
        self.recalculate(sheet_idx);
    }

    /// Insert `count` blank rows at `at_row` (1-based), shifting existing rows down.
    pub fn insert_rows(&mut self, sheet_idx: usize, at_row: u32, count: u32) {
        let Some(sheet) = self.sheets.get_mut(sheet_idx) else {
            return;
        };
        let mut new_cells: BTreeMap<String, SheetCell> = BTreeMap::new();
        for (key, mut cell) in std::mem::take(&mut sheet.cells) {
            let Ok((r, c)) = parse_key(&key) else {
                new_cells.insert(key, cell);
                continue;
            };
            if r >= at_row {
                // Adjust formula references in the cell before moving it
                if let Some(ref formula) = cell.formula.clone() {
                    let adjusted = adjust_formula_references_advanced(
                        formula,
                        0,
                        0,
                        Some((at_row, count as i32)),
                        None,
                    );
                    cell.formula = Some(adjusted.clone());
                    cell.raw_value = adjusted;
                }
                new_cells.insert(format!("{}:{}", r + count, c), cell);
            } else {
                // Adjust formula references that point into shifted region
                if cell.formula.is_some() {
                    let formula = cell.raw_value.clone();
                    let adjusted = adjust_formula_references_advanced(
                        &formula,
                        0,
                        0,
                        Some((at_row, count as i32)),
                        None,
                    );
                    cell.formula = Some(adjusted.clone());
                    cell.raw_value = adjusted;
                }
                new_cells.insert(key, cell);
            }
        }
        sheet.cells = new_cells;
        self.invalidate_cell_value_cache(sheet_idx);
        self.recalculate(sheet_idx);
    }

    /// Delete `count` rows starting at `at_row` (1-based), shifting rows above up.
    pub fn delete_rows(&mut self, sheet_idx: usize, at_row: u32, count: u32) {
        let Some(sheet) = self.sheets.get_mut(sheet_idx) else {
            return;
        };
        let end_row = at_row + count; // exclusive
        let mut new_cells: BTreeMap<String, SheetCell> = BTreeMap::new();
        for (key, mut cell) in std::mem::take(&mut sheet.cells) {
            let Ok((r, c)) = parse_key(&key) else {
                new_cells.insert(key, cell);
                continue;
            };
            if r >= at_row && r < end_row {
                // Row is being deleted — drop it
                continue;
            } else if r >= end_row {
                // Shift up
                if cell.formula.is_some() {
                    let formula = cell.raw_value.clone();
                    let adjusted = adjust_formula_references_advanced(
                        &formula,
                        0,
                        0,
                        Some((at_row, -(count as i32))),
                        None,
                    );
                    cell.formula = Some(adjusted.clone());
                    cell.raw_value = adjusted;
                }
                new_cells.insert(format!("{}:{}", r - count, c), cell);
            } else {
                // Above deleted range — still adjust formula refs pointing into shifted area
                if cell.formula.is_some() {
                    let formula = cell.raw_value.clone();
                    let adjusted = adjust_formula_references_advanced(
                        &formula,
                        0,
                        0,
                        Some((at_row, -(count as i32))),
                        None,
                    );
                    cell.formula = Some(adjusted.clone());
                    cell.raw_value = adjusted;
                }
                new_cells.insert(key, cell);
            }
        }
        sheet.cells = new_cells;
        self.invalidate_cell_value_cache(sheet_idx);
        self.recalculate(sheet_idx);
    }

    /// Insert `count` blank columns at `at_col` (1-based), shifting existing columns right.
    pub fn insert_cols(&mut self, sheet_idx: usize, at_col: u32, count: u32) {
        let Some(sheet) = self.sheets.get_mut(sheet_idx) else {
            return;
        };
        let mut new_cells: BTreeMap<String, SheetCell> = BTreeMap::new();
        for (key, mut cell) in std::mem::take(&mut sheet.cells) {
            let Ok((r, c)) = parse_key(&key) else {
                new_cells.insert(key, cell);
                continue;
            };
            if c >= at_col {
                if let Some(ref formula) = cell.formula.clone() {
                    let adjusted = adjust_formula_references_advanced(
                        formula,
                        0,
                        0,
                        None,
                        Some((at_col, count as i32)),
                    );
                    cell.formula = Some(adjusted.clone());
                    cell.raw_value = adjusted;
                }
                new_cells.insert(format!("{}:{}", r, c + count), cell);
            } else {
                if cell.formula.is_some() {
                    let formula = cell.raw_value.clone();
                    let adjusted = adjust_formula_references_advanced(
                        &formula,
                        0,
                        0,
                        None,
                        Some((at_col, count as i32)),
                    );
                    cell.formula = Some(adjusted.clone());
                    cell.raw_value = adjusted;
                }
                new_cells.insert(key, cell);
            }
        }
        sheet.cells = new_cells;
        self.invalidate_cell_value_cache(sheet_idx);
        self.recalculate(sheet_idx);
    }

    /// Delete `count` columns starting at `at_col` (1-based), shifting columns right leftward.
    pub fn delete_cols(&mut self, sheet_idx: usize, at_col: u32, count: u32) {
        let Some(sheet) = self.sheets.get_mut(sheet_idx) else {
            return;
        };
        let end_col = at_col + count; // exclusive
        let mut new_cells: BTreeMap<String, SheetCell> = BTreeMap::new();
        for (key, mut cell) in std::mem::take(&mut sheet.cells) {
            let Ok((r, c)) = parse_key(&key) else {
                new_cells.insert(key, cell);
                continue;
            };
            if c >= at_col && c < end_col {
                continue;
            } else if c >= end_col {
                if cell.formula.is_some() {
                    let formula = cell.raw_value.clone();
                    let adjusted = adjust_formula_references_advanced(
                        &formula,
                        0,
                        0,
                        None,
                        Some((at_col, -(count as i32))),
                    );
                    cell.formula = Some(adjusted.clone());
                    cell.raw_value = adjusted;
                }
                new_cells.insert(format!("{}:{}", r, c - count), cell);
            } else {
                if cell.formula.is_some() {
                    let formula = cell.raw_value.clone();
                    let adjusted = adjust_formula_references_advanced(
                        &formula,
                        0,
                        0,
                        None,
                        Some((at_col, -(count as i32))),
                    );
                    cell.formula = Some(adjusted.clone());
                    cell.raw_value = adjusted;
                }
                new_cells.insert(key, cell);
            }
        }
        sheet.cells = new_cells;
        self.invalidate_cell_value_cache(sheet_idx);
        self.recalculate(sheet_idx);
    }

    /// Set freeze pane boundaries for a sheet.
    pub fn set_freeze(&mut self, sheet_idx: usize, freeze_rows: u32, freeze_cols: u32) {
        if let Some(sheet) = self.sheets.get_mut(sheet_idx) {
            sheet.freeze_rows = freeze_rows;
            sheet.freeze_cols = freeze_cols;
        }
    }

    pub fn add_named_range(
        &mut self,
        name: String,
        range_str: String,
        sheet: Option<String>,
    ) -> Result<(), String> {
        if name.trim().is_empty() {
            return Err("Name cannot be empty".to_string());
        }
        if self.named_ranges.iter().any(|nr| nr.name == name) {
            return Err("Named range already exists".to_string());
        }
        self.named_ranges.push(NamedRange {
            name,
            range_str,
            sheet,
        });
        for idx in 0..self.sheets.len() {
            self.invalidate_cell_value_cache(idx);
        }
        for idx in 0..self.sheets.len() {
            self.recalculate(idx);
        }
        Ok(())
    }

    pub fn remove_named_range(&mut self, name: &str) {
        let len_before = self.named_ranges.len();
        self.named_ranges.retain(|nr| nr.name != name);
        if self.named_ranges.len() < len_before {
            for idx in 0..self.sheets.len() {
                self.invalidate_cell_value_cache(idx);
            }
            for idx in 0..self.sheets.len() {
                self.recalculate(idx);
            }
        }
    }

    pub fn get_named_range(&self, name: &str) -> Option<&NamedRange> {
        self.named_ranges.iter().find(|nr| nr.name == name)
    }

    pub fn list_named_ranges(&self) -> Vec<NamedRange> {
        self.named_ranges.clone()
    }
}

pub fn expand_named_ranges(formula: &str, named_ranges: &[NamedRange]) -> String {
    let has_eq = formula.starts_with('=');
    let text = if has_eq { &formula[1..] } else { formula };

    let mut result = String::new();
    if has_eq {
        result.push('=');
    }

    let mut chars = text.chars().peekable();

    while let Some(&c) = chars.peek() {
        if c == '"' {
            if let Some(ch) = chars.next() {
                result.push(ch);
            }
            while let Some(&sc) = chars.peek() {
                result.push(sc);
                chars.next();
                if sc == '"' {
                    break;
                }
            }
        } else if c.is_ascii_alphabetic() || c == '_' {
            let mut ident = String::new();
            while let Some(&ic) = chars.peek() {
                if ic.is_alphanumeric() || ic == '_' {
                    ident.push(ic);
                    chars.next();
                } else {
                    break;
                }
            }

            if chars.peek() == Some(&'(') {
                result.push_str(&ident);
            } else {
                let mut found = false;
                for nr in named_ranges {
                    if nr.name == ident {
                        result.push_str(&nr.range_str);
                        found = true;
                        break;
                    }
                }
                if !found {
                    result.push_str(&ident);
                }
            }
        } else {
            result.push(c);
            chars.next();
        }
    }
    result
}

fn extract_local_dependencies(expr: &Expr) -> HashSet<(u32, u32)> {
    let mut deps = HashSet::new();

    fn collect(expr: &Expr, deps: &mut HashSet<(u32, u32)>) {
        match expr {
            Expr::CellRef {
                sheet: None,
                row,
                col,
            } => {
                deps.insert((*row, *col));
            }
            Expr::RangeRef {
                sheet: None,
                start_row,
                start_col,
                end_row,
                end_col,
            } => {
                for row in *start_row..=*end_row {
                    for col in *start_col..=*end_col {
                        deps.insert((row, col));
                    }
                }
            }
            Expr::Binary { left, right, .. } => {
                collect(left, deps);
                collect(right, deps);
            }
            Expr::FunctionCall { args, .. } => {
                for arg in args {
                    collect(arg, deps);
                }
            }
            Expr::CellRef { sheet: Some(_), .. } | Expr::RangeRef { sheet: Some(_), .. } => {}
            Expr::Literal(_) => {}
        }
    }

    collect(expr, &mut deps);
    deps
}

fn resolve_sheet_index(sheet_name: &str, sheet_names: &[String]) -> Option<usize> {
    sheet_names
        .iter()
        .position(|name| name.eq_ignore_ascii_case(sheet_name))
}

fn extract_external_dependencies(
    expr: &Expr,
    local_sheet_idx: usize,
    sheet_names: &[String],
) -> HashSet<(usize, u32, u32)> {
    let mut deps = HashSet::new();

    fn collect(
        expr: &Expr,
        local_sheet_idx: usize,
        sheet_names: &[String],
        deps: &mut HashSet<(usize, u32, u32)>,
    ) {
        match expr {
            Expr::CellRef { sheet, row, col } => {
                let target_sheet = sheet
                    .as_deref()
                    .and_then(|name| resolve_sheet_index(name, sheet_names))
                    .unwrap_or(local_sheet_idx);
                if target_sheet != local_sheet_idx {
                    deps.insert((target_sheet, *row, *col));
                }
            }
            Expr::RangeRef {
                sheet,
                start_row,
                start_col,
                end_row,
                end_col,
            } => {
                let target_sheet = sheet
                    .as_deref()
                    .and_then(|name| resolve_sheet_index(name, sheet_names))
                    .unwrap_or(local_sheet_idx);
                if target_sheet != local_sheet_idx {
                    for row in *start_row..=*end_row {
                        for col in *start_col..=*end_col {
                            deps.insert((target_sheet, row, col));
                        }
                    }
                }
            }
            Expr::Binary { left, right, .. } => {
                collect(left, local_sheet_idx, sheet_names, deps);
                collect(right, local_sheet_idx, sheet_names, deps);
            }
            Expr::FunctionCall { args, .. } => {
                for arg in args {
                    collect(arg, local_sheet_idx, sheet_names, deps);
                }
            }
            _ => {}
        }
    }

    collect(expr, local_sheet_idx, sheet_names, &mut deps);
    deps
}

#[derive(Debug, Clone, Copy)]
pub struct ParseKeyError;

pub fn parse_key(key: &str) -> Result<(u32, u32), ParseKeyError> {
    let parts: Vec<&str> = key.split(':').collect();
    if parts.len() == 2 {
        let r: u32 = parts[0].parse().map_err(|_| ParseKeyError)?;
        let c: u32 = parts[1].parse().map_err(|_| ParseKeyError)?;
        Ok((r, c))
    } else {
        Err(ParseKeyError)
    }
}

fn formula_error_from_display(display: &str) -> Option<FormulaError> {
    match display {
        "#DIV/0!" => Some(FormulaError::DivZero),
        "#REF!" => Some(FormulaError::Ref),
        "#NAME?" => Some(FormulaError::Name),
        "#VALUE!" => Some(FormulaError::Value),
        "#CYCLE!" => Some(FormulaError::Cycle),
        "#N/A" => Some(FormulaError::Na),
        _ => None,
    }
}

fn literal_to_formula_value(raw: &str) -> FormulaValue {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return FormulaValue::Empty;
    }
    if trimmed.eq_ignore_ascii_case("TRUE") {
        return FormulaValue::Boolean(true);
    }
    if trimmed.eq_ignore_ascii_case("FALSE") {
        return FormulaValue::Boolean(false);
    }
    if let Some(err) = formula_error_from_display(trimmed) {
        return FormulaValue::Error(err);
    }
    if let Ok(n) = trimmed.parse::<f64>() {
        return FormulaValue::Number(n);
    }
    FormulaValue::String(trimmed.to_string())
}

fn cell_to_formula_value(cell: &SheetCell) -> FormulaValue {
    if cell.formula.is_some() {
        if let Some(err) = formula_error_from_display(&cell.display_value) {
            return FormulaValue::Error(err);
        }
        if cell.display_value.eq_ignore_ascii_case("TRUE") {
            return FormulaValue::Boolean(true);
        }
        if cell.display_value.eq_ignore_ascii_case("FALSE") {
            return FormulaValue::Boolean(false);
        }
        if let Ok(n) = cell.display_value.parse::<f64>() {
            return FormulaValue::Number(n);
        }
        if !cell.display_value.is_empty() && !cell.display_value.starts_with('#') {
            return FormulaValue::String(cell.display_value.clone());
        }
        return FormulaValue::Empty;
    }
    literal_to_formula_value(&cell.raw_value)
}

fn format_formula_value(value: &FormulaValue) -> String {
    match value {
        FormulaValue::Number(n) => n.to_string(),
        FormulaValue::String(s) => s.clone(),
        FormulaValue::Boolean(b) => b.to_string().to_uppercase(),
        FormulaValue::Error(e) => e.to_str().to_string(),
        FormulaValue::Empty => String::new(),
        FormulaValue::Array(vals, _, _) => vals
            .first()
            .map(format_formula_value)
            .unwrap_or_default(),
    }
}

impl WorkbookModel {
    pub fn merge_cells(&mut self, sheet_idx: usize, range: CellRange) -> bool {
        let Some(sheet) = self.sheets.get_mut(sheet_idx) else {
            return false;
        };
        if range.end_row < range.start_row || range.end_col < range.start_col {
            return false;
        }
        if range.start_row == range.end_row && range.start_col == range.end_col {
            return false;
        }
        // Remove overlapping merges first.
        sheet.merges.retain(|m| {
            m.end_row < range.start_row
                || m.start_row > range.end_row
                || m.end_col < range.start_col
                || m.start_col > range.end_col
        });
        sheet.merges.push(MergeRange {
            start_row: range.start_row,
            end_row: range.end_row,
            start_col: range.start_col,
            end_col: range.end_col,
        });
        true
    }

    pub fn unmerge_cells(&mut self, sheet_idx: usize, row: u32, col: u32) -> bool {
        let Some(sheet) = self.sheets.get_mut(sheet_idx) else {
            return false;
        };
        let before = sheet.merges.len();
        sheet.merges.retain(|m| {
            !(row >= m.start_row && row <= m.end_row && col >= m.start_col && col <= m.end_col)
        });
        sheet.merges.len() != before
    }

    pub fn set_auto_filter(&mut self, sheet_idx: usize, range: CellRange) -> bool {
        let Some(sheet) = self.sheets.get_mut(sheet_idx) else {
            return false;
        };
        sheet.auto_filter = Some(AutoFilterState {
            enabled: true,
            start_row: range.start_row,
            end_row: range.end_row,
            start_col: range.start_col,
            end_col: range.end_col,
            column_filters: BTreeMap::new(),
        });
        true
    }

    pub fn clear_auto_filter(&mut self, sheet_idx: usize) {
        if let Some(sheet) = self.sheets.get_mut(sheet_idx) {
            sheet.auto_filter = None;
        }
    }

    pub fn set_column_filter_values(
        &mut self,
        sheet_idx: usize,
        col: u32,
        values: Vec<String>,
    ) -> bool {
        let Some(sheet) = self.sheets.get_mut(sheet_idx) else {
            return false;
        };
        let Some(filter) = sheet.auto_filter.as_mut() else {
            return false;
        };
        if values.is_empty() {
            filter.column_filters.remove(&col);
        } else {
            filter.column_filters.insert(col, values);
        }
        true
    }

    pub fn filtered_rows_hidden(&self, sheet_idx: usize) -> HashSet<u32> {
        let mut hidden = HashSet::new();
        let Some(sheet) = self.sheets.get(sheet_idx) else {
            return hidden;
        };
        let Some(filter) = sheet.auto_filter.as_ref() else {
            return hidden;
        };
        if !filter.enabled || filter.column_filters.is_empty() {
            return hidden;
        }
        for row in (filter.start_row + 1)..=filter.end_row {
            let mut keep = true;
            for (col, allowed) in &filter.column_filters {
                let key = format!("{}:{}", row, col);
                let value = sheet
                    .cells
                    .get(&key)
                    .map(|c| c.display_value.clone())
                    .unwrap_or_default();
                if !allowed.iter().any(|a| a == &value) {
                    keep = false;
                    break;
                }
            }
            if !keep {
                hidden.insert(row);
            }
        }
        hidden
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedRef {
    pub sheet: Option<String>,
    pub col: u32,
    pub row: u32,
    pub col_abs: bool,
    pub row_abs: bool,
}

pub fn col_to_letters(mut col: u32) -> String {
    let mut letters = Vec::new();
    while col > 0 {
        let rem = ((col - 1) % 26) as u8;
        letters.push((b'A' + rem) as char);
        col = (col - 1) / 26;
    }
    letters.into_iter().rev().collect()
}

pub fn parse_a1_reference_details(s: &str) -> Option<ParsedRef> {
    let (sheet, ref_part) = if let Some(idx) = s.rfind('!') {
        (Some(s[..idx].to_string()), &s[idx + 1..])
    } else {
        (None, s)
    };

    let mut chars = ref_part.chars().peekable();

    let col_abs = if chars.peek() == Some(&'$') {
        chars.next();
        true
    } else {
        false
    };

    let mut col_str = String::new();
    while let Some(&c) = chars.peek() {
        if c.is_ascii_alphabetic() {
            col_str.push(c.to_ascii_uppercase());
            chars.next();
        } else {
            break;
        }
    }

    if col_str.is_empty() {
        return None;
    }

    let row_abs = if chars.peek() == Some(&'$') {
        chars.next();
        true
    } else {
        false
    };

    let mut row_str = String::new();
    while let Some(&c) = chars.peek() {
        if c.is_ascii_digit() {
            row_str.push(c);
            chars.next();
        } else {
            break;
        }
    }

    if row_str.is_empty() {
        return None;
    }

    if chars.peek().is_some() {
        return None;
    }

    let mut col: u32 = 0;
    for c in col_str.chars() {
        col = col * 26 + ((c as u32) - ('A' as u32) + 1);
    }
    let row: u32 = row_str.parse().ok()?;

    Some(ParsedRef {
        sheet,
        col,
        row,
        col_abs,
        row_abs,
    })
}

impl std::fmt::Display for ParsedRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(ref sheet) = self.sheet {
            write!(f, "{}!", sheet)?;
        }
        if self.col_abs {
            write!(f, "$")?;
        }
        write!(f, "{}", col_to_letters(self.col))?;
        if self.row_abs {
            write!(f, "$")?;
        }
        write!(f, "{}", self.row)
    }
}

pub fn adjust_ref(
    r: &mut ParsedRef,
    delta_row: i32,
    delta_col: i32,
    insert_row: Option<(u32, i32)>,
    insert_col: Option<(u32, i32)>,
) -> bool {
    if delta_col != 0 && !r.col_abs {
        let new_col = r.col as i32 + delta_col;
        if new_col < 1 {
            return false;
        }
        r.col = new_col as u32;
    }

    if delta_row != 0 && !r.row_abs {
        let new_row = r.row as i32 + delta_row;
        if new_row < 1 {
            return false;
        }
        r.row = new_row as u32;
    }

    if let Some((at_row, count)) = insert_row {
        if count > 0 {
            if r.row >= at_row {
                r.row += count as u32;
            }
        } else if count < 0 {
            let delete_count = (-count) as u32;
            if r.row >= at_row && r.row < at_row + delete_count {
                return false;
            } else if r.row >= at_row + delete_count {
                r.row -= delete_count;
            }
        }
    }

    if let Some((at_col, count)) = insert_col {
        if count > 0 {
            if r.col >= at_col {
                r.col += count as u32;
            }
        } else if count < 0 {
            let delete_count = (-count) as u32;
            if r.col >= at_col && r.col < at_col + delete_count {
                return false;
            } else if r.col >= at_col + delete_count {
                r.col -= delete_count;
            }
        }
    }

    true
}

pub fn adjust_formula_references_advanced(
    formula: &str,
    delta_row: i32,
    delta_col: i32,
    insert_row: Option<(u32, i32)>,
    insert_col: Option<(u32, i32)>,
) -> String {
    let has_eq = formula.starts_with('=');
    let text = if has_eq { &formula[1..] } else { formula };

    let mut result = String::new();
    if has_eq {
        result.push('=');
    }

    let mut chars = text.chars().peekable();

    while let Some(&c) = chars.peek() {
        if c == '"' {
            if let Some(ch) = chars.next() {
                result.push(ch);
            }
            while let Some(&sc) = chars.peek() {
                result.push(sc);
                chars.next();
                if sc == '"' {
                    break;
                }
            }
        } else if c.is_ascii_alphabetic() || c == '_' || c == '$' {
            let mut ident = String::new();
            while let Some(&ic) = chars.peek() {
                if ic.is_alphanumeric() || ic == '_' || ic == '!' || ic == '$' {
                    ident.push(ic);
                    chars.next();
                } else {
                    break;
                }
            }

            if chars.peek() == Some(&':') {
                chars.next(); // consume ':'
                let mut end_part = String::new();
                while let Some(&ec) = chars.peek() {
                    if ec.is_alphanumeric() || ec == '_' || ec == '$' {
                        end_part.push(ec);
                        chars.next();
                    } else {
                        break;
                    }
                }

                if let (Some(mut start_ref), Some(mut end_ref)) = (
                    parse_a1_reference_details(&ident),
                    parse_a1_reference_details(&end_part),
                ) {
                    let start_ok =
                        adjust_ref(&mut start_ref, delta_row, delta_col, insert_row, insert_col);
                    let end_ok =
                        adjust_ref(&mut end_ref, delta_row, delta_col, insert_row, insert_col);

                    let start_str = if start_ok {
                        start_ref.to_string()
                    } else {
                        "#REF!".to_string()
                    };
                    let end_str = if end_ok {
                        end_ref.to_string()
                    } else {
                        "#REF!".to_string()
                    };

                    result.push_str(&format!("{}:{}", start_str, end_str));
                } else {
                    result.push_str(&ident);
                    result.push(':');
                    result.push_str(&end_part);
                }
            } else if chars.peek() == Some(&'(') {
                result.push_str(&ident);
            } else if let Some(mut single_ref) = parse_a1_reference_details(&ident) {
                let ok = adjust_ref(
                    &mut single_ref,
                    delta_row,
                    delta_col,
                    insert_row,
                    insert_col,
                );
                if ok {
                    result.push_str(&single_ref.to_string());
                } else {
                    result.push_str("#REF!");
                }
            } else {
                result.push_str(&ident);
            }
        } else {
            result.push(c);
            chars.next();
        }
    }

    result
}

pub fn adjust_formula_references(formula: &str, delta_row: i32, delta_col: i32) -> String {
    adjust_formula_references_advanced(formula, delta_row, delta_col, None, None)
}

struct WorkbookCellProvider<'a> {
    sheet_names: &'a [String],
    values: &'a [HashMap<(u32, u32), FormulaValue>],
    sheet_idx: usize,
}

impl<'a> CellProvider for WorkbookCellProvider<'a> {
    fn get_cell_value(&self, sheet_name: Option<&str>, row: u32, col: u32) -> FormulaValue {
        let target_index = sheet_name
            .and_then(|name| {
                self.sheet_names
                    .iter()
                    .position(|sheet| sheet.eq_ignore_ascii_case(name))
            })
            .unwrap_or(self.sheet_idx);
        if let Some(values) = self.values.get(target_index) {
            if let Some(value) = values.get(&(row, col)) {
                return value.clone();
            }
        }
        FormulaValue::Empty
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recalculates_chained_formulas() {
        let mut workbook = WorkbookModel::new_default();
        workbook.set_cell_value(0, 1, 1, "10".to_string());
        workbook.set_cell_value(0, 1, 2, "=A1*2".to_string());
        workbook.set_cell_value(0, 1, 3, "=B1+5".to_string());
        assert_eq!(workbook.sheets[0].cells["1:3"].display_value, "25");
    }

    #[test]
    fn resolves_cross_sheet_references() {
        let mut workbook = WorkbookModel::new_default();
        workbook.sheets.push(SheetData::new("sheet-2", "Totals"));
        workbook.set_cell_value(0, 1, 1, "42".to_string());
        workbook.set_cell_value(1, 1, 1, "=Sheet1!A1".to_string());
        assert_eq!(workbook.sheets[1].cells["1:1"].display_value, "42");

        workbook.set_cell_value(0, 1, 1, "99".to_string());
        assert_eq!(workbook.sheets[1].cells["1:1"].display_value, "99");
    }

    #[test]
    fn reports_formula_cycles() {
        let mut workbook = WorkbookModel::new_default();
        workbook.set_cell_value(0, 1, 1, "=B1".to_string());
        workbook.set_cell_value(0, 1, 2, "=A1".to_string());
        assert_eq!(workbook.sheets[0].cells["1:1"].display_value, "#CYCLE!");
        assert_eq!(workbook.sheets[0].cells["1:2"].display_value, "#CYCLE!");
    }

    #[test]
    fn fills_numbers_and_sorts_ranges() {
        let mut workbook = WorkbookModel::new_default();
        workbook.set_cell_value(0, 1, 1, "3".to_string());
        workbook.fill_series(0, 1, 1, 3, 1);
        assert_eq!(workbook.sheets[0].cells["3:1"].raw_value, "5");
        workbook.set_cell_value(0, 1, 2, "B".to_string());
        workbook.set_cell_value(0, 2, 2, "A".to_string());
        workbook.sort_range(
            0,
            CellRange {
                start_row: 1,
                end_row: 2,
                start_col: 1,
                end_col: 2,
            },
            2,
            true,
        );
        assert_eq!(workbook.sheets[0].cells["1:2"].raw_value, "A");
    }

    #[test]
    fn merges_and_filters_cells() {
        let mut workbook = WorkbookModel::new_default();
        assert!(workbook.merge_cells(
            0,
            CellRange {
                start_row: 1,
                end_row: 2,
                start_col: 1,
                end_col: 2,
            }
        ));
        assert_eq!(workbook.sheets[0].merges.len(), 1);
        assert!(workbook.unmerge_cells(0, 1, 1));
        assert!(workbook.sheets[0].merges.is_empty());
        assert!(workbook.set_auto_filter(
            0,
            CellRange {
                start_row: 1,
                end_row: 3,
                start_col: 1,
                end_col: 2,
            }
        ));
        workbook.set_cell_value(0, 2, 1, "keep".to_string());
        workbook.set_cell_value(0, 3, 1, "drop".to_string());
        workbook.set_column_filter_values(0, 1, vec!["keep".to_string()]);
        let hidden = workbook.filtered_rows_hidden(0);
        assert!(hidden.contains(&3));
        assert!(!hidden.contains(&2));
    }

    #[test]
    fn recalculates_formulas_when_value_cell_changes() {
        let mut workbook = WorkbookModel::new_default();
        workbook.set_cell_value(0, 1, 1, "10".to_string());
        workbook.set_cell_value(0, 1, 2, "=A1 * 2".to_string());
        assert_eq!(workbook.sheets[0].cells["1:2"].display_value, "20");

        workbook.set_cell_value(0, 1, 1, "25".to_string());
        assert_eq!(workbook.sheets[0].cells["1:2"].display_value, "50");
    }

    #[test]
    fn preserves_cell_style_on_value_edit() {
        use crate::cell::CellStyle;
        let mut workbook = WorkbookModel::new_default();
        workbook.set_cell_value(0, 1, 1, "100".to_string());

        let style = CellStyle {
            bold: Some(true),
            italic: None,
            underline: None,
            font_color: Some("#ff0000".to_string()),
            bg_color: None,
            align: None,
            format: None,
            wrap: None,
            v_align: None,
        };
        if let Some(cell) = workbook.sheets[0].cells.get_mut("1:1") {
            cell.style = Some(style.clone());
        }

        workbook.set_cell_value(0, 1, 1, "200".to_string());

        let cell = workbook.sheets[0].cells.get("1:1").unwrap();
        assert_eq!(cell.raw_value, "200");
        assert!(cell.style.is_some());
        assert_eq!(cell.style.as_ref().unwrap().bold, Some(true));
        assert_eq!(
            cell.style.as_ref().unwrap().font_color.as_deref(),
            Some("#ff0000")
        );
    }

    #[test]
    fn test_adjust_formula_references() {
        assert_eq!(
            adjust_formula_references("=SUM(A1:B10) + $C$5 + D$2 + $E3", 1, 2),
            "=SUM(C2:D11) + $C$5 + F$2 + $E4"
        );
        assert_eq!(
            adjust_formula_references("=IF(TRUE, A1, \"A1\")", 1, 0),
            "=IF(TRUE, A2, \"A1\")"
        );
        assert_eq!(
            adjust_formula_references("=Sheet1!$A$1 + Sheet2!B2", 1, 1),
            "=Sheet1!$A$1 + Sheet2!C3"
        );
    }

    #[test]
    fn test_adjust_formula_references_advanced_insert_delete() {
        // Insert row at row 2 (+1 count)
        assert_eq!(
            adjust_formula_references_advanced("=A1 + A2 + $A$2", 0, 0, Some((2, 1)), None),
            "=A1 + A3 + $A$3"
        );
        // Delete row at row 2 (-1 count)
        assert_eq!(
            adjust_formula_references_advanced("=A1 + A2 + A3", 0, 0, Some((2, -1)), None),
            "=A1 + #REF! + A2"
        );
    }

    #[test]
    fn test_sort_range_multi_numeric_aware() {
        let mut workbook = WorkbookModel::new_default();
        // Set up 3 rows in col 1 and col 2
        // Row 1: "Dept A", "10"
        // Row 2: "Dept A", "2"
        // Row 3: "Dept B", "1"
        workbook.set_cell_value(0, 1, 1, "Dept A".to_string());
        workbook.set_cell_value(0, 1, 2, "10".to_string());

        workbook.set_cell_value(0, 2, 1, "Dept A".to_string());
        workbook.set_cell_value(0, 2, 2, "2".to_string());

        workbook.set_cell_value(0, 3, 1, "Dept B".to_string());
        workbook.set_cell_value(0, 3, 2, "1".to_string());

        // Sort by col 1 asc, then col 2 asc (numeric)
        workbook.sort_range_multi(
            0,
            CellRange {
                start_row: 1,
                end_row: 3,
                start_col: 1,
                end_col: 2,
            },
            &[(1, true), (2, true)],
        );

        // Expect:
        // Row 1: Dept A, 2 (numeric 2 < 10)
        // Row 2: Dept A, 10
        // Row 3: Dept B, 1
        assert_eq!(workbook.sheets[0].cells["1:2"].display_value, "2");
        assert_eq!(workbook.sheets[0].cells["2:2"].display_value, "10");
        assert_eq!(workbook.sheets[0].cells["3:2"].display_value, "1");
    }

    #[test]
    fn test_insert_rows_shifts_cells_down() {
        let mut wb = WorkbookModel::new_default();
        wb.set_cell_value(0, 1, 1, "A".to_string());
        wb.set_cell_value(0, 2, 1, "B".to_string());
        // Insert 2 rows at row 1 (everything >= row 1 shifts down by 2)
        wb.insert_rows(0, 1, 2);
        // Original row 1 -> row 3, row 2 -> row 4
        assert!(
            wb.sheets[0].cells.contains_key("3:1"),
            "row 1 should shift to row 3"
        );
        assert!(
            wb.sheets[0].cells.contains_key("4:1"),
            "row 2 should shift to row 4"
        );
        assert!(
            !wb.sheets[0].cells.contains_key("1:1"),
            "row 1 should be empty after insert"
        );
        assert_eq!(wb.sheets[0].cells["3:1"].raw_value, "A");
        assert_eq!(wb.sheets[0].cells["4:1"].raw_value, "B");
    }

    #[test]
    fn test_delete_rows_removes_and_shifts() {
        let mut wb = WorkbookModel::new_default();
        wb.set_cell_value(0, 1, 1, "keep-above".to_string());
        wb.set_cell_value(0, 2, 1, "delete-me".to_string());
        wb.set_cell_value(0, 3, 1, "shift-up".to_string());
        // Delete row 2 (1 row)
        wb.delete_rows(0, 2, 1);
        assert!(wb.sheets[0].cells.contains_key("1:1"), "row 1 intact");
        assert_eq!(wb.sheets[0].cells["1:1"].raw_value, "keep-above");
        // Row 3 should now be at row 2
        assert!(
            wb.sheets[0].cells.contains_key("2:1"),
            "row 3 shifted to row 2"
        );
        assert_eq!(wb.sheets[0].cells["2:1"].raw_value, "shift-up");
        // Original row 2 should be gone (no cell at that key with "delete-me")
        // (row 2 now contains "shift-up")
        assert_ne!(
            wb.sheets[0].cells.get("2:1").map(|c| c.raw_value.as_str()),
            Some("delete-me")
        );
    }

    #[test]
    fn test_insert_cols_shifts_cells_right() {
        let mut wb = WorkbookModel::new_default();
        wb.set_cell_value(0, 1, 1, "col1".to_string());
        wb.set_cell_value(0, 1, 2, "col2".to_string());
        // Insert 1 column at col 1 (everything >= col 1 shifts right)
        wb.insert_cols(0, 1, 1);
        assert!(wb.sheets[0].cells.contains_key("1:2"), "old col 1 -> col 2");
        assert!(wb.sheets[0].cells.contains_key("1:3"), "old col 2 -> col 3");
        assert!(
            !wb.sheets[0].cells.contains_key("1:1"),
            "col 1 should be empty"
        );
        assert_eq!(wb.sheets[0].cells["1:2"].raw_value, "col1");
        assert_eq!(wb.sheets[0].cells["1:3"].raw_value, "col2");
    }

    #[test]
    fn test_delete_cols_removes_and_shifts() {
        let mut wb = WorkbookModel::new_default();
        wb.set_cell_value(0, 1, 1, "keep".to_string());
        wb.set_cell_value(0, 1, 2, "drop".to_string());
        wb.set_cell_value(0, 1, 3, "shift-left".to_string());
        wb.delete_cols(0, 2, 1);
        assert_eq!(wb.sheets[0].cells["1:1"].raw_value, "keep");
        // col 3 should have shifted to col 2
        assert!(
            wb.sheets[0].cells.contains_key("1:2"),
            "col 3 shifted to col 2"
        );
        assert_eq!(wb.sheets[0].cells["1:2"].raw_value, "shift-left");
        // col 2 "drop" should be gone
        assert_ne!(
            wb.sheets[0].cells.get("1:2").map(|c| c.raw_value.as_str()),
            Some("drop")
        );
    }

    #[test]
    fn test_freeze_fields_serialize() {
        let mut wb = WorkbookModel::new_default();
        wb.set_freeze(0, 3, 2);
        let sheet = &wb.sheets[0];
        assert_eq!(sheet.freeze_rows, 3);
        assert_eq!(sheet.freeze_cols, 2);
        // Verify round-trip through JSON
        let json = serde_json::to_string(sheet).expect("serialize");
        let restored: SheetData = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored.freeze_rows, 3);
        assert_eq!(restored.freeze_cols, 2);
    }
}


#[test]
fn test_expand_named_ranges() {
    let ranges = vec![NamedRange { name: "Tax".into(), range_str: "B2".into(), sheet: None }];
    assert_eq!(expand_named_ranges("=SUM(Tax, 5)", &ranges), "=SUM(B2, 5)");
    assert_eq!(expand_named_ranges("=Tax * 2", &ranges), "=B2 * 2");
}

