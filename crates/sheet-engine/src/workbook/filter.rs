use crate::cell::{AutoFilterState, MergeRange};
use super::model::{CellRange, WorkbookModel};
use std::collections::{BTreeMap, HashSet};

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
