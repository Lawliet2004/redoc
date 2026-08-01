use crate::cell::SheetCell;
use super::formula_rewrite::{expand_named_ranges, extract_local_dependencies, extract_external_dependencies};
use super::model::{RecalcPlan, SheetData, WorkbookModel};
use redoc_formula::FormulaError;
use redoc_formula::{eval_expr, parse_formula, CellProvider, Expr, FormulaValue};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;


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

    pub(crate) fn invalidate_cell_value_cache(&mut self, sheet_idx: usize) {
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
    use crate::workbook::formula_rewrite::{adjust_formula_references, adjust_formula_references_advanced};
    use crate::workbook::model::CellRange;

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

#[cfg(test)]
mod expand_named_ranges_tests {
    use crate::workbook::formula_rewrite::expand_named_ranges;
    use crate::workbook::model::NamedRange;

    #[test]
    fn test_expand_named_ranges() {
        let ranges = vec![NamedRange { name: "Tax".into(), range_str: "B2".into(), sheet: None }];
        assert_eq!(expand_named_ranges("=SUM(Tax, 5)", &ranges), "=SUM(B2, 5)");
        assert_eq!(expand_named_ranges("=Tax * 2", &ranges), "=B2 * 2");
    }
}

