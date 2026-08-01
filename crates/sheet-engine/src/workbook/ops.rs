use crate::cell::SheetCell;
use super::formula_rewrite::{adjust_formula_references, adjust_formula_references_advanced};
use super::model::{CellRange, NamedRange, WorkbookModel};
use super::workbook_core::parse_key;
use std::collections::BTreeMap;

impl WorkbookModel {
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
