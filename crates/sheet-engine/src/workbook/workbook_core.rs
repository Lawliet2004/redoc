use super::formula_rewrite::{
    expand_named_ranges, extract_external_dependencies, extract_local_dependencies,
};
use super::model::{ArraySpill, RecalcPlan, SheetData, WorkbookModel, MAX_ARRAY_SPILL_CELLS};
use crate::cell::SheetCell;
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
            formula_values_valid: vec![false],
            recalc_plans: vec![None],
            cross_sheet_dependents: HashMap::new(),
            cross_sheet_index_valid: false,
        }
    }

    pub fn set_cell_value(&mut self, sheet_idx: usize, row: u32, col: u32, raw_value: String) {
        let key = format!("{}:{}", row, col);
        let next_formula = raw_value.starts_with('=').then(|| raw_value.clone());
        let formula_changed = self
            .sheets
            .get(sheet_idx)
            .and_then(|sheet| sheet.cells.get(&key))
            .and_then(|cell| cell.formula.clone())
            != next_formula;
        if let Some(sheet) = self.sheets.get_mut(sheet_idx) {
            if raw_value.trim().is_empty() {
                sheet.cells.remove(&key);
            } else {
                let existing_style = sheet.cells.get(&key).and_then(|c| c.style.clone());
                let formula = next_formula;
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
        if formula_changed {
            self.invalidate_cell_value_cache(sheet_idx);
        } else {
            // Literal edits do not change the parsed formulas or dependency
            // graph. Update only the edited provider value and keep both
            // plans hot instead of cloning/rebuilding thousands of entries.
            self.invalidate_formula_values(sheet_idx);
            self.update_cell_value_cache_entry(sheet_idx, row, col);
        }
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
        if dirty_cell.is_none()
            && self
                .formula_values_valid
                .get(sheet_idx)
                .copied()
                .unwrap_or(false)
        {
            return HashSet::new();
        }
        self.ensure_recalc_plan(sheet_idx);
        let Some(plan) = self.recalc_plans.get(sheet_idx).and_then(Option::as_ref) else {
            return HashSet::new();
        };
        let Some(order) = plan.order.clone() else {
            if let Some(sheet) = self.sheets.get_mut(sheet_idx) {
                sheet.spills.clear();
            }
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
            if let Some(valid) = self.formula_values_valid.get_mut(sheet_idx) {
                *valid = true;
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
            // A literal edit can either block or unblock a dynamic-array
            // destination without appearing in the formula dependency graph.
            // Re-evaluate the bounded dynamic-function subset so collision
            // state remains correct without abandoning incremental recalc for
            // ordinary formulas.
            if let Some(sheet) = self.sheets.get(sheet_idx) {
                let dynamic_formula_present = sheet.cells.values().any(|cell| {
                    cell.formula
                        .as_deref()
                        .is_some_and(is_dynamic_array_formula)
                });
                for (key, cell) in &sheet.cells {
                    let Ok(coord) = parse_key(key) else {
                        continue;
                    };
                    if dynamic_formula_present
                        || cell
                            .formula
                            .as_deref()
                            .is_some_and(is_dynamic_array_formula)
                    {
                        affected.insert(coord);
                    }
                }
                if dynamic_formula_present {
                    affected.extend(plan.formulas.keys().copied());
                }
            }
            affected
        });

        // Remove stale materialized ranges before evaluating affected formulas.
        // A full recalculation rebuilds every spill; a dirty recalculation keeps
        // unrelated ranges hot for incremental updates.
        let mut changed_spill_coords = HashSet::new();
        if let Some(sheet) = self.sheets.get_mut(sheet_idx) {
            for spill in &sheet.spills {
                if affected
                    .as_ref()
                    .is_none_or(|cells| cells.contains(&(spill.origin_row, spill.origin_col)))
                {
                    let count = (spill.rows as usize)
                        .saturating_mul(spill.cols as usize)
                        .min(spill.values.len())
                        .min(MAX_ARRAY_SPILL_CELLS);
                    for index in 0..count {
                        if let Some(coord) =
                            spill_coordinate(spill.origin_row, spill.origin_col, index, spill.cols)
                        {
                            changed_spill_coords.insert(coord);
                        }
                    }
                }
            }
            if let Some(affected) = &affected {
                sheet
                    .spills
                    .retain(|spill| !affected.contains(&(spill.origin_row, spill.origin_col)));
            } else {
                sheet.spills.clear();
            }
        }

        // Spill neighbors are cached only for the duration of a calculation;
        // never let a removed range masquerade as a literal cell on the next
        // edit or load.
        let actual_coords: HashSet<_> = self.sheets[sheet_idx]
            .cells
            .keys()
            .filter_map(|key| parse_key(key).ok())
            .collect();
        self.cell_value_cache[sheet_idx].retain(|coord, _| actual_coords.contains(coord));

        let sheet_names: Vec<String> = self.sheets.iter().map(|sheet| sheet.name.clone()).collect();
        let mut spill_values = collect_spill_values(&self.sheets);
        let has_dynamic_formula = plan.formulas.keys().any(|coord| {
            self.sheets[sheet_idx]
                .cells
                .get(&format!("{}:{}", coord.0, coord.1))
                .and_then(|cell| cell.formula.as_deref())
                .is_some_and(is_dynamic_array_formula)
        });
        let mut recalculated = changed_spill_coords;
        let passes = if has_dynamic_formula { 2 } else { 1 };
        for _pass in 0..passes {
            if _pass > 0 {
                // A formula feeding a spill can change its shape between
                // passes. Rebuild the lookup and drop prior synthetic cache
                // entries so removed neighbors cannot leak stale values.
                spill_values = collect_spill_values(&self.sheets);
                self.cell_value_cache[sheet_idx].retain(|coord, _| actual_coords.contains(coord));
            }
            for coord in &order {
                let coord = *coord;
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
                let evaluated = {
                    let provider = WorkbookCellProvider {
                        sheet_names: &sheet_names,
                        values: &self.cell_value_cache,
                        spill_values: &spill_values,
                        sheet_idx,
                    };
                    eval_expr(&ast, &provider)
                };
                let value = if let FormulaValue::Array(_, rows, cols) = &evaluated {
                    let cell_count = (*rows as usize).saturating_mul(*cols as usize);
                    let blocked = cell_count > 0
                        && cell_count <= MAX_ARRAY_SPILL_CELLS
                        && self.sheets.get(sheet_idx).is_some_and(|sheet| {
                            (1..cell_count).any(|index| {
                                spill_coordinate(coord.0, coord.1, index, *cols).is_some_and(
                                    |(row, col)| sheet.cells.contains_key(&format!("{row}:{col}")),
                                )
                            })
                        });
                    if blocked {
                        FormulaValue::Error(FormulaError::Spill)
                    } else {
                        evaluated
                    }
                } else {
                    evaluated
                };
                if let FormulaValue::Array(values, rows, cols) = &value {
                    let count = (*rows as usize)
                        .saturating_mul(*cols as usize)
                        .min(values.len())
                        .min(MAX_ARRAY_SPILL_CELLS);
                    for (index, value) in values.iter().take(count).enumerate() {
                        let Some((row, col)) = spill_coordinate(coord.0, coord.1, index, *cols)
                        else {
                            continue;
                        };
                        spill_values.insert((sheet_idx, row, col), value.clone());
                        if index > 0 {
                            self.cell_value_cache[sheet_idx].insert((row, col), value.clone());
                            recalculated.insert((row, col));
                        }
                    }
                }
                let display = format_formula_value(&value);
                let spill = match &value {
                    FormulaValue::Array(values, rows, cols)
                        if *rows > 0
                            && *cols > 0
                            && (*rows as usize).saturating_mul(*cols as usize)
                                <= MAX_ARRAY_SPILL_CELLS =>
                    {
                        let count = (*rows as usize)
                            .saturating_mul(*cols as usize)
                            .min(values.len())
                            .min(MAX_ARRAY_SPILL_CELLS);
                        Some(ArraySpill {
                            origin_row: coord.0,
                            origin_col: coord.1,
                            rows: *rows,
                            cols: *cols,
                            values: values[..count].iter().map(format_formula_value).collect(),
                        })
                    }
                    _ => None,
                };
                self.cell_value_cache[sheet_idx].insert(coord, value);
                if let Some(sheet) = self.sheets.get_mut(sheet_idx) {
                    if let Some(cell) = sheet.cells.get_mut(&format!("{}:{}", coord.0, coord.1)) {
                        cell.display_value = display;
                    }
                    sheet
                        .spills
                        .retain(|existing| (existing.origin_row, existing.origin_col) != coord);
                    if let Some(spill) = spill {
                        sheet.spills.push(spill);
                    }
                }
                recalculated.insert(coord);
            }
        }
        if let Some(valid) = self.formula_values_valid.get_mut(sheet_idx) {
            *valid = true;
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
        self.ensure_cache_slots();
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

    fn ensure_cache_slots(&mut self) {
        if self.cell_value_cache.len() != self.sheets.len() {
            self.cell_value_cache
                .resize_with(self.sheets.len(), HashMap::new);
            self.cell_value_cache_valid.resize(self.sheets.len(), false);
            self.formula_values_valid.resize(self.sheets.len(), false);
        }
    }

    fn invalidate_formula_values(&mut self, sheet_idx: usize) {
        self.ensure_cache_slots();
        if let Some(valid) = self.formula_values_valid.get_mut(sheet_idx) {
            *valid = false;
        }
    }

    fn update_cell_value_cache_entry(&mut self, sheet_idx: usize, row: u32, col: u32) {
        if !self
            .cell_value_cache_valid
            .get(sheet_idx)
            .copied()
            .unwrap_or(false)
        {
            return;
        }
        let key = format!("{row}:{col}");
        let value = self
            .sheets
            .get(sheet_idx)
            .and_then(|sheet| sheet.cells.get(&key))
            .map(cell_to_formula_value);
        if let Some(cache) = self.cell_value_cache.get_mut(sheet_idx) {
            if let Some(value) = value {
                cache.insert((row, col), value);
            } else {
                cache.remove(&(row, col));
            }
        }
    }

    pub(crate) fn invalidate_cell_value_cache(&mut self, sheet_idx: usize) {
        self.invalidate_formula_values(sheet_idx);
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
        "#SPILL!" => Some(FormulaError::Spill),
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
        FormulaValue::Array(vals, _, _) => {
            vals.first().map(format_formula_value).unwrap_or_default()
        }
    }
}

fn is_dynamic_array_formula(formula: &str) -> bool {
    let upper = formula.to_ascii_uppercase();
    [
        "FILTER(",
        "RANDARRAY(",
        "SEQUENCE(",
        "SORT(",
        "HSTACK(",
        "TAKE(",
        "DROP(",
        "CHOOSECOLS(",
        "CHOOSEROWS(",
        "TRANSPOSE(",
        "TOCOL(",
        "TOROW(",
        "UNIQUE(",
        "VSTACK(",
        "SORTBY(",
        "WRAPCOLS(",
        "WRAPROWS(",
    ]
    .iter()
    .any(|name| upper.contains(name))
}

fn spill_coordinate(
    origin_row: u32,
    origin_col: u32,
    index: usize,
    cols: u32,
) -> Option<(u32, u32)> {
    let width = cols.max(1) as usize;
    let row_offset = u32::try_from(index / width).ok()?;
    let col_offset = u32::try_from(index % width).ok()?;
    Some((
        origin_row.checked_add(row_offset)?,
        origin_col.checked_add(col_offset)?,
    ))
}

fn collect_spill_values(sheets: &[SheetData]) -> HashMap<(usize, u32, u32), FormulaValue> {
    let mut values = HashMap::new();
    for (source_sheet_idx, sheet) in sheets.iter().enumerate() {
        for spill in &sheet.spills {
            let count = (spill.rows as usize)
                .saturating_mul(spill.cols as usize)
                .min(spill.values.len())
                .min(MAX_ARRAY_SPILL_CELLS);
            for (index, value) in spill.values.iter().take(count).enumerate() {
                let Some((row, col)) =
                    spill_coordinate(spill.origin_row, spill.origin_col, index, spill.cols)
                else {
                    continue;
                };
                values.insert(
                    (source_sheet_idx, row, col),
                    literal_to_formula_value(value),
                );
            }
        }
    }
    values
}

struct WorkbookCellProvider<'a> {
    sheet_names: &'a [String],
    values: &'a [HashMap<(u32, u32), FormulaValue>],
    spill_values: &'a HashMap<(usize, u32, u32), FormulaValue>,
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
        if let Some(value) = self.spill_values.get(&(target_index, row, col)) {
            return value.clone();
        }
        FormulaValue::Empty
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::workbook::formula_rewrite::{
        adjust_formula_references, adjust_formula_references_advanced,
    };
    use crate::workbook::model::CellRange;
    use crate::{ScenarioCellChange, ScenarioModel, SlicerModel};

    #[test]
    fn recalculates_chained_formulas() {
        let mut workbook = WorkbookModel::new_default();
        workbook.set_cell_value(0, 1, 1, "10".to_string());
        workbook.set_cell_value(0, 1, 2, "=A1*2".to_string());
        workbook.set_cell_value(0, 1, 3, "=B1+5".to_string());
        assert_eq!(workbook.sheets[0].cells["1:3"].display_value, "25");
    }

    #[test]
    fn recalculates_statistical_compatibility_formulas() {
        let mut workbook = WorkbookModel::new_default();
        workbook.set_cell_value(0, 1, 1, "10".to_string());
        workbook.set_cell_value(0, 1, 2, "20".to_string());
        workbook.set_cell_value(0, 1, 3, "=RANK.EQ(B1,A1:B1)".to_string());
        workbook.set_cell_value(0, 1, 4, "=QUARTILE.INC(A1:B1,1)".to_string());
        workbook.set_cell_value(0, 1, 5, "=CORREL(A1:B1,A1:B1)".to_string());
        workbook.set_cell_value(0, 1, 6, "=SUMSQ(A1:B1)".to_string());
        assert_eq!(workbook.sheets[0].cells["1:3"].display_value, "1");
        assert_eq!(workbook.sheets[0].cells["1:4"].display_value, "12.5");
        assert_eq!(workbook.sheets[0].cells["1:5"].display_value, "1");
        assert_eq!(workbook.sheets[0].cells["1:6"].display_value, "500");
    }

    #[test]
    fn materializes_bounded_dynamic_array_spills() {
        let mut workbook = WorkbookModel::new_default();
        workbook.set_cell_value(0, 1, 1, "1".to_string());
        workbook.set_cell_value(0, 1, 2, "2".to_string());
        workbook.set_cell_value(0, 2, 1, "3".to_string());
        workbook.set_cell_value(0, 2, 2, "4".to_string());
        workbook.set_cell_value(0, 1, 4, "=TRANSPOSE(A1:B2)".to_string());

        let spill = workbook.sheets[0]
            .spills
            .iter()
            .find(|spill| spill.origin_row == 1 && spill.origin_col == 4)
            .expect("array formula should expose a spill range");
        assert_eq!((spill.rows, spill.cols), (2, 2));
        assert_eq!(spill.values, vec!["1", "3", "2", "4"]);

        workbook.set_cell_value(0, 1, 1, "9".to_string());
        let updated = workbook.sheets[0]
            .spills
            .iter()
            .find(|spill| spill.origin_row == 1 && spill.origin_col == 4)
            .expect("dependent spill should remain after dirty recalc");
        assert_eq!(updated.values[0], "9");

        let encoded = serde_json::to_string(&workbook).expect("spills should serialize");
        let decoded: WorkbookModel =
            serde_json::from_str(&encoded).expect("spills should deserialize");
        assert_eq!(decoded.sheets[0].spills, workbook.sheets[0].spills);
    }

    #[test]
    fn materializes_sequence_spills_with_expected_values() {
        let mut workbook = WorkbookModel::new_default();
        workbook.set_cell_value(0, 1, 1, "=SEQUENCE(2,3,10,2)".to_string());

        let spill = workbook.sheets[0]
            .spills
            .iter()
            .find(|spill| spill.origin_row == 1 && spill.origin_col == 1)
            .expect("SEQUENCE should expose a spill range");
        assert_eq!((spill.rows, spill.cols), (2, 3));
        assert_eq!(spill.values, vec!["10", "12", "14", "16", "18", "20"]);
    }

    #[test]
    fn materializes_randarray_spills_with_bounded_shape() {
        let mut workbook = WorkbookModel::new_default();
        workbook.set_cell_value(0, 1, 1, "=RANDARRAY(2,2,10,20,TRUE)".to_string());

        let spill = workbook.sheets[0]
            .spills
            .iter()
            .find(|spill| spill.origin_row == 1 && spill.origin_col == 1)
            .expect("RANDARRAY should expose a spill range");
        assert_eq!((spill.rows, spill.cols), (2, 2));
        assert_eq!(spill.values.len(), 4);
        assert!(spill.values.iter().all(|value| {
            value
                .parse::<f64>()
                .map(|number| (10.0..=20.0).contains(&number) && number.fract() == 0.0)
                .unwrap_or(false)
        }));
    }

    #[test]
    fn materializes_stacked_dynamic_arrays_with_padding() {
        let mut workbook = WorkbookModel::new_default();
        workbook.set_cell_value(0, 1, 1, "=HSTACK(SEQUENCE(2),VSTACK(3,4,5))".to_string());

        let spill = workbook.sheets[0]
            .spills
            .iter()
            .find(|spill| spill.origin_row == 1 && spill.origin_col == 1)
            .expect("HSTACK should expose a spill range");
        assert_eq!((spill.rows, spill.cols), (3, 2));
        assert_eq!(spill.values, vec!["1", "3", "2", "4", "", "5"]);
    }

    #[test]
    fn materializes_selected_reference_array_spills() {
        let mut workbook = WorkbookModel::new_default();
        workbook.set_cell_value(0, 1, 1, "=CHOOSECOLS(SEQUENCE(2,3),3,1)".to_string());

        let spill = workbook.sheets[0]
            .spills
            .iter()
            .find(|spill| spill.origin_row == 1 && spill.origin_col == 1)
            .expect("CHOOSECOLS should expose a spill range");
        assert_eq!((spill.rows, spill.cols), (2, 2));
        assert_eq!(spill.values, vec!["3", "1", "6", "4"]);
    }

    #[test]
    fn materializes_sortby_spills_in_key_order() {
        let mut workbook = WorkbookModel::new_default();
        workbook.set_cell_value(0, 1, 1, "A".to_string());
        workbook.set_cell_value(0, 1, 2, "30".to_string());
        workbook.set_cell_value(0, 2, 1, "B".to_string());
        workbook.set_cell_value(0, 2, 2, "10".to_string());
        workbook.set_cell_value(0, 3, 1, "C".to_string());
        workbook.set_cell_value(0, 3, 2, "20".to_string());
        workbook.set_cell_value(0, 1, 4, "=SORTBY(A1:B3,B1:B3,1)".to_string());

        let spill = workbook.sheets[0]
            .spills
            .iter()
            .find(|spill| spill.origin_row == 1 && spill.origin_col == 4)
            .expect("SORTBY should expose a spill range");
        assert_eq!((spill.rows, spill.cols), (3, 2));
        assert_eq!(spill.values, vec!["B", "10", "C", "20", "A", "30"]);
    }

    #[test]
    fn materializes_flattened_array_spills_with_column_scan() {
        let mut workbook = WorkbookModel::new_default();
        workbook.set_cell_value(0, 1, 1, "1".to_string());
        workbook.set_cell_value(0, 1, 2, "2".to_string());
        workbook.set_cell_value(0, 2, 1, "3".to_string());
        workbook.set_cell_value(0, 2, 2, "4".to_string());
        workbook.set_cell_value(0, 1, 4, "=TOCOL(A1:B2,0,TRUE)".to_string());

        let spill = workbook.sheets[0]
            .spills
            .iter()
            .find(|spill| spill.origin_row == 1 && spill.origin_col == 4)
            .expect("TOCOL should expose a spill range");
        assert_eq!((spill.rows, spill.cols), (4, 1));
        assert_eq!(spill.values, vec!["1", "3", "2", "4"]);
    }

    #[test]
    fn materializes_wrapped_array_spills_with_safe_padding() {
        let mut workbook = WorkbookModel::new_default();
        workbook.set_cell_value(0, 1, 1, "=WRAPROWS(SEQUENCE(5),2)".to_string());

        let spill = workbook.sheets[0]
            .spills
            .iter()
            .find(|spill| spill.origin_row == 1 && spill.origin_col == 1)
            .expect("WRAPROWS should expose a spill range");
        assert_eq!((spill.rows, spill.cols), (3, 2));
        assert_eq!(spill.values, vec!["1", "2", "3", "4", "5", "#N/A"]);
    }

    #[test]
    fn reports_spill_collision_and_rechecks_after_blocker_is_cleared() {
        let mut workbook = WorkbookModel::new_default();
        workbook.set_cell_value(0, 1, 1, "1".to_string());
        workbook.set_cell_value(0, 1, 2, "2".to_string());
        workbook.set_cell_value(0, 2, 1, "3".to_string());
        workbook.set_cell_value(0, 2, 2, "4".to_string());
        workbook.set_cell_value(0, 1, 5, "blocked".to_string());
        workbook.set_cell_value(0, 1, 4, "=TRANSPOSE(A1:B2)".to_string());

        assert_eq!(workbook.sheets[0].cells["1:4"].display_value, "#SPILL!");
        assert!(workbook.sheets[0].spills.is_empty());

        workbook.set_cell_value(0, 1, 5, String::new());
        assert_eq!(workbook.sheets[0].cells["1:4"].display_value, "1");
        assert_eq!(
            workbook.sheets[0].spills[0].values,
            vec!["1", "3", "2", "4"]
        );
    }

    #[test]
    fn formulas_can_read_materialized_spill_neighbors() {
        let mut workbook = WorkbookModel::new_default();
        workbook.set_cell_value(0, 1, 1, "1".to_string());
        workbook.set_cell_value(0, 1, 2, "2".to_string());
        workbook.set_cell_value(0, 2, 1, "3".to_string());
        workbook.set_cell_value(0, 2, 2, "4".to_string());
        workbook.set_cell_value(0, 1, 4, "=TRANSPOSE(A1:B2)".to_string());
        workbook.set_cell_value(0, 4, 1, "=E2+1".to_string());

        assert_eq!(workbook.sheets[0].cells["4:1"].display_value, "5");

        workbook.set_cell_value(0, 2, 2, "9".to_string());
        assert_eq!(workbook.sheets[0].cells["4:1"].display_value, "10");
    }

    #[test]
    fn cross_sheet_formulas_follow_spill_neighbor_changes() {
        let mut workbook = WorkbookModel::new_default();
        workbook.sheets.push(SheetData::new("sheet-2", "Summary"));
        workbook.set_cell_value(0, 1, 1, "1".to_string());
        workbook.set_cell_value(0, 1, 2, "2".to_string());
        workbook.set_cell_value(0, 2, 1, "3".to_string());
        workbook.set_cell_value(0, 2, 2, "4".to_string());
        workbook.set_cell_value(0, 1, 4, "=TRANSPOSE(A1:B2)".to_string());
        workbook.set_cell_value(1, 1, 1, "=Sheet1!E2+1".to_string());

        assert_eq!(workbook.sheets[1].cells["1:1"].display_value, "5");

        workbook.set_cell_value(0, 2, 2, "9".to_string());
        assert_eq!(workbook.sheets[1].cells["1:1"].display_value, "10");
    }

    #[test]
    fn scenario_metadata_round_trips_through_workbook_serialization() {
        let mut workbook = WorkbookModel::new_default();
        workbook.sheets[0].scenarios.push(ScenarioModel {
            id: "scenario-1".to_string(),
            name: "Conservative case".to_string(),
            changes: vec![ScenarioCellChange {
                row: 2,
                col: 3,
                raw_value: "125".to_string(),
            }],
        });
        workbook.sheets[0].slicers.push(SlicerModel {
            id: "slicer-1".to_string(),
            title: "Region".to_string(),
            source_range: CellRange {
                start_row: 1,
                end_row: 4,
                start_col: 1,
                end_col: 2,
            },
            column: 1,
            selected_values: vec!["East".to_string()],
        });

        let encoded = serde_json::to_string(&workbook).expect("scenario metadata should serialize");
        let decoded: WorkbookModel =
            serde_json::from_str(&encoded).expect("scenario metadata should deserialize");
        assert_eq!(decoded.sheets[0].scenarios, workbook.sheets[0].scenarios);
        assert_eq!(decoded.sheets[0].slicers, workbook.sheets[0].slicers);
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
    fn repeated_recalculate_reuses_valid_formula_values() {
        let mut workbook = WorkbookModel::new_default();
        workbook.set_cell_value(0, 1, 1, "10".to_string());
        workbook.set_cell_value(0, 1, 2, "=A1 * 2".to_string());
        assert!(workbook.formula_values_valid[0]);

        workbook.recalculate(0);
        assert!(workbook.formula_values_valid[0]);
        assert_eq!(workbook.sheets[0].cells["1:2"].display_value, "20");
        assert!(workbook.recalc_plans[0].is_some());

        workbook.set_cell_value(0, 1, 1, "30".to_string());
        assert!(workbook.formula_values_valid[0]);
        assert!(workbook.recalc_plans[0].is_some());
        assert_eq!(workbook.sheets[0].cells["1:2"].display_value, "60");
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
            validation: None,
            hyperlink: None,
            image: None,
            ..Default::default()
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
    fn typography_and_decimals_survive_serialization() {
        use crate::cell::CellStyle;
        let mut workbook = WorkbookModel::new_default();
        workbook.set_cell_value(0, 1, 1, "3.14159".to_string());
        if let Some(cell) = workbook.sheets[0].cells.get_mut("1:1") {
            cell.style = Some(CellStyle {
                font_family: Some("Georgia".to_string()),
                font_size: Some(14.0),
                decimals: Some(3),
                format: Some("number".to_string()),
                ..Default::default()
            });
        }
        let json = serde_json::to_string(&workbook).expect("serialize workbook");
        let restored: WorkbookModel = serde_json::from_str(&json).expect("deserialize workbook");
        let style = restored.sheets[0]
            .cells
            .get("1:1")
            .and_then(|cell| cell.style.as_ref())
            .expect("style survives round-trip");
        assert_eq!(style.font_family.as_deref(), Some("Georgia"));
        assert_eq!(style.font_size, Some(14.0));
        assert_eq!(style.decimals, Some(3));

        // Older files (no typography fields) still deserialize.
        let legacy = r#"{"bold":true}"#;
        let legacy_style: CellStyle = serde_json::from_str(legacy).expect("legacy style loads");
        assert_eq!(legacy_style.bold, Some(true));
        assert_eq!(legacy_style.font_family, None);
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
    fn absolute_references_keep_dollars_through_all_adjust_kinds() {
        // Fill/copy deltas must NOT move absolutized components…
        assert_eq!(adjust_formula_references("=$A$1", 3, 3), "=$A$1");
        assert_eq!(adjust_formula_references("=$A1+A$1", 3, 3), "=$A4+D$1");
        // …but insert/delete shifts everything in/after the inserted span,
        // matching Excel (insert above $A$1 moves it to $A$2).
        assert_eq!(
            adjust_formula_references_advanced("=$A$1+$B$2", 0, 0, Some((1, 1)), None),
            "=$A$2+$B$3"
        );
        // Deleting the referenced row yields #REF! even for absolutes.
        assert_eq!(
            adjust_formula_references_advanced("=$A$1", 0, 0, Some((1, -1)), None),
            "=#REF!"
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
        let ranges = vec![NamedRange {
            name: "Tax".into(),
            range_str: "B2".into(),
            sheet: None,
        }];
        assert_eq!(expand_named_ranges("=SUM(Tax, 5)", &ranges), "=SUM(B2, 5)");
        assert_eq!(expand_named_ranges("=Tax * 2", &ranges), "=B2 * 2");
    }

    #[test]
    fn does_not_expand_named_ranges_shadowed_by_let_bindings() {
        let ranges = vec![NamedRange {
            name: "amount".to_string(),
            range_str: "B2".to_string(),
            sheet: None,
        }];
        assert_eq!(
            expand_named_ranges("=LET(amount,10,amount+5)", &ranges),
            "=LET(amount,10,amount+5)"
        );
    }
}
