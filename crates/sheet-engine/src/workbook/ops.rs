use super::formula_rewrite::{adjust_formula_references, adjust_formula_references_advanced};
use super::model::{CellRange, NamedRange, WorkbookModel};
use super::workbook_core::parse_key;
use crate::cell::SheetCell;
use std::collections::BTreeMap;

fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let yoe = year - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let days = days + 719468;
    let era = if days >= 0 { days } else { days - 146096 } / 146097;
    let doe = days - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    (if month <= 2 { year + 1 } else { year }, month, day)
}

const EXCEL_EPOCH_DAYS: i64 = -25569;

fn serial_to_ymd(serial: i64) -> (i64, i64, i64) {
    civil_from_days(serial + EXCEL_EPOCH_DAYS)
}

fn ymd_to_serial(year: i64, month: i64, day: i64) -> i64 {
    days_from_civil(year, month, day) - EXCEL_EPOCH_DAYS
}

fn days_in_month(year: i64, month: i64) -> i64 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 => 29,
        2 => 28,
        _ => 30,
    }
}

/// Adds `count` calendar units to an Excel day serial, clamping the day like Excel.
fn add_date_units(serial: i64, count: i64, unit: DateUnit) -> i64 {
    let (year, month, day) = serial_to_ymd(serial);
    match unit {
        DateUnit::Day => serial + count,
        DateUnit::Month => {
            let total = year * 12 + (month - 1) + count;
            let next_year = total.div_euclid(12);
            let next_month = total.rem_euclid(12) + 1;
            let clamped = day.min(days_in_month(next_year, next_month));
            ymd_to_serial(next_year, next_month, clamped)
        }
        DateUnit::Year => {
            let next_year = year + count;
            let clamped = if month == 2 && day == 29 && days_in_month(next_year, 2) == 28 {
                28
            } else {
                day
            };
            ymd_to_serial(next_year, month, clamped)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum DateUnit {
    Day,
    Month,
    Year,
}

#[derive(Debug, Clone)]
enum SeriesValue {
    Number(f64),
    /// Text with a trailing number: "Item1" → prefix "Item", number 1.
    TextNumber {
        prefix: String,
        number: f64,
    },
    /// Day serial with a "date" cell format.
    DateSerial(f64),
    Other,
}

/// Trailing-digit split for text+number suffix series ("Item1" → "Item" + 1).
fn split_trailing_number(text: &str) -> Option<(&str, f64)> {
    let trimmed = text.trim_end_matches(|c: char| c.is_ascii_digit());
    if trimmed.is_empty() || trimmed.len() == text.len() {
        return None;
    }
    text[trimmed.len()..]
        .parse::<f64>()
        .ok()
        .map(|number| (trimmed, number))
}

fn parse_series_value(cell: &SheetCell) -> SeriesValue {
    let is_date = cell
        .style
        .as_ref()
        .and_then(|style| style.format.as_deref())
        .is_some_and(|format| format == "date");
    if let Ok(number) = cell.raw_value.parse::<f64>() {
        if is_date && number.fract() == 0.0 && number.abs() < 3_000_000.0 {
            return SeriesValue::DateSerial(number);
        }
        return SeriesValue::Number(number);
    }
    if let Some(serial) = parse_iso_date(&cell.raw_value) {
        return SeriesValue::DateSerial(serial);
    }
    if let Some((prefix, number)) = split_trailing_number(&cell.raw_value) {
        return SeriesValue::TextNumber {
            prefix: prefix.to_string(),
            number,
        };
    }
    SeriesValue::Other
}

/// "2024-03-05" (and 2024/03/05) to an Excel day serial; text-formatted dates.
fn parse_iso_date(text: &str) -> Option<f64> {
    let text = text.trim();
    let bytes = text.as_bytes();
    if bytes.len() != 10 || bytes[4] != bytes[7] {
        return None;
    }
    let separator = bytes[4];
    if separator != b'-' && separator != b'/' {
        return None;
    }
    let year: i64 = text.get(0..4)?.parse().ok()?;
    let month: i64 = text.get(5..7)?.parse().ok()?;
    let day: i64 = text.get(8..10)?.parse().ok()?;
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) || day > days_in_month(year, month) {
        return None;
    }
    Some(ymd_to_serial(year, month, day) as f64)
}

fn same_series_kind(a: &SeriesValue, b: &SeriesValue) -> bool {
    matches!(
        (a, b),
        (SeriesValue::Number(_), SeriesValue::Number(_))
            | (SeriesValue::DateSerial(_), SeriesValue::DateSerial(_))
            | (
                SeriesValue::TextNumber { .. },
                SeriesValue::TextNumber { .. }
            )
    )
}

/// Text-series counters render without trailing ".0" ("Item2", not "Item2.0").
fn format_number(value: f64) -> String {
    if value.fract() == 0.0 && value.abs() < 1e15 {
        format!("{}", value as i64)
    } else {
        value.to_string()
    }
}

fn format_serial_as_iso(serial: i64) -> String {
    let (year, month, day) = serial_to_ymd(serial);
    format!("{year:04}-{month:02}-{day:02}")
}

/// How the source run extends: numeric step, calendar step, text counter, or copy.
#[derive(Debug, Clone)]
enum SeriesRule {
    Number { step: f64 },
    Date { step: i64, unit: DateUnit },
    TextNumber { prefix: String, step: f64 },
    Copy,
}

fn detect_rule(seed: &[SheetCell]) -> SeriesRule {
    let values: Vec<SeriesValue> = seed.iter().map(parse_series_value).collect();
    let last = values.last();
    let prev = if values.len() >= 2 {
        Some(&values[values.len() - 2])
    } else {
        None
    };
    match (prev, last) {
        (Some(SeriesValue::Number(a)), Some(SeriesValue::Number(b))) => {
            SeriesRule::Number { step: b - a }
        }
        (Some(SeriesValue::DateSerial(a)), Some(SeriesValue::DateSerial(b))) => {
            let day_diff = (*b - *a) as i64;
            let (y0, m0, d0) = serial_to_ymd(*a as i64);
            let (y1, m1, d1) = serial_to_ymd(*b as i64);
            // Month steps keep the day-of-month (Feb 29 ← Jan 31 counts when
            // the target day is the month's last day, matching Excel's clamp).
            let month_like =
                (m0 != m1 || y0 != y1) && (d0 == d1 || (d1 == days_in_month(y1, m1) && d0 > d1));
            if month_like && !(m0 == m1 && y0 == y1) {
                let months = (y1 - y0) * 12 + (m1 - m0);
                if months % 12 == 0 && m0 == m1 {
                    SeriesRule::Date {
                        step: months / 12,
                        unit: DateUnit::Year,
                    }
                } else {
                    SeriesRule::Date {
                        step: months,
                        unit: DateUnit::Month,
                    }
                }
            } else {
                SeriesRule::Date {
                    step: day_diff,
                    unit: DateUnit::Day,
                }
            }
        }
        (
            Some(SeriesValue::TextNumber { prefix, number }),
            Some(SeriesValue::TextNumber {
                prefix: prefix2,
                number: number2,
            }),
        ) if *prefix == *prefix2 => SeriesRule::TextNumber {
            prefix: prefix.clone(),
            step: number2 - number,
        },
        (None | Some(_), Some(SeriesValue::Number(_))) => SeriesRule::Number { step: 1.0 },
        (None | Some(_), Some(SeriesValue::DateSerial(_))) => SeriesRule::Date {
            step: 1,
            unit: DateUnit::Day,
        },
        (None | Some(_), Some(SeriesValue::TextNumber { prefix, .. })) => SeriesRule::TextNumber {
            prefix: prefix.clone(),
            step: 1.0,
        },
        _ => SeriesRule::Copy,
    }
}

impl WorkbookModel {
    pub fn fill_series(
        &mut self,
        sheet_idx: usize,
        source_row: u32,
        source_col: u32,
        target_row: u32,
        target_col: u32,
    ) {
        let Some(sheet) = self.sheets.get(sheet_idx) else {
            return;
        };
        let Some(source) = sheet
            .cells
            .get(&format!("{}:{}", source_row, source_col))
            .cloned()
        else {
            return;
        };
        if source_row == target_row && source_col == target_col {
            return;
        }
        let start_row = source_row.min(target_row);
        let end_row = source_row.max(target_row);
        let start_col = source_col.min(target_col);
        let end_col = source_col.max(target_col);
        let fill_dr = target_row as i64 - source_row as i64;
        let fill_dc = target_col as i64 - source_col as i64;
        let vertical = fill_dr.abs() >= fill_dc.abs();

        // Seed run: contiguous same-kind cells ending at the source, walking
        // up/left in axis order (the values the extension follows).
        let mut seed: Vec<SheetCell> = vec![source.clone()];
        let mut walk_row = source_row as i32 - 1;
        let mut walk_col = source_col as i32 - 1;
        while seed.len() < 8
            && if vertical {
                walk_row >= 1
            } else {
                walk_col >= 1
            }
        {
            let (r, c) = if vertical {
                (walk_row, source_col as i32)
            } else {
                (source_row as i32, walk_col)
            };
            let Some(cell) = sheet.cells.get(&format!("{}:{}", r, c)) else {
                break;
            };
            // The seed must be one homogeneous run: stop at a header or a
            // different value kind so "Qty" above "3" does not become the step.
            let Some(last_seed) = seed.last() else {
                break;
            };
            if !same_series_kind(&parse_series_value(cell), &parse_series_value(last_seed)) {
                break;
            }
            seed.push(cell.clone());
            walk_row -= 1;
            walk_col -= 1;
        }
        seed.reverse();

        let rule = if source.formula.is_some() {
            SeriesRule::Copy
        } else {
            detect_rule(&seed)
        };
        // Seed is always non-empty (initialized with source above); this is
        // a safety fallback that should never trigger.
        let last_seed = seed.last().unwrap_or(&source);
        let last_value = parse_series_value(last_seed);
        // Text-formatted dates keep producing text dates, not bare serials.
        let date_as_text =
            matches!(rule, SeriesRule::Date { .. }) && source.raw_value.parse::<f64>().is_err();

        let Some(sheet) = self.sheets.get_mut(sheet_idx) else {
            return;
        };
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
                } else {
                    // Series-axis distance from the source (positive = down/
                    // right). Filling up/left extrapolates to negative steps.
                    let axis_distance = if vertical {
                        row as i64 - source_row as i64
                    } else {
                        col as i64 - source_col as i64
                    };
                    let signed_steps = axis_distance as f64;
                    match (&rule, &last_value) {
                        (SeriesRule::Number { step }, SeriesValue::Number(base)) => {
                            let value = base + *step * signed_steps;
                            cell.raw_value = value.to_string();
                            cell.display_value = cell.raw_value.clone();
                            cell.formula = None;
                        }
                        (SeriesRule::Date { step, unit }, SeriesValue::DateSerial(base)) => {
                            let count = (*step as f64 * signed_steps) as i64;
                            let serial = add_date_units(*base as i64, count, *unit);
                            cell.raw_value = if date_as_text {
                                format_serial_as_iso(serial)
                            } else {
                                serial.to_string()
                            };
                            cell.display_value = cell.raw_value.clone();
                            cell.formula = None;
                        }
                        (
                            SeriesRule::TextNumber { prefix, step },
                            SeriesValue::TextNumber { number, .. },
                        ) => {
                            let value = number + *step * signed_steps;
                            cell.raw_value = format!("{}{}", prefix, format_number(value));
                            cell.display_value = cell.raw_value.clone();
                            cell.formula = None;
                        }
                        _ => {}
                    }
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

    pub fn set_row_hidden(&mut self, sheet_idx: usize, rows: &[u32], hidden: bool) {
        if let Some(sheet) = self.sheets.get_mut(sheet_idx) {
            for row in rows {
                if *row == 0 {
                    continue;
                }
                if hidden {
                    sheet.hidden_rows.insert(*row);
                } else {
                    sheet.hidden_rows.remove(row);
                }
            }
        }
    }

    pub fn set_col_hidden(&mut self, sheet_idx: usize, cols: &[u32], hidden: bool) {
        if let Some(sheet) = self.sheets.get_mut(sheet_idx) {
            for col in cols {
                if *col == 0 {
                    continue;
                }
                if hidden {
                    sheet.hidden_cols.insert(*col);
                } else {
                    sheet.hidden_cols.remove(col);
                }
            }
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

#[cfg(test)]
mod tests {
    use super::super::model::CellRange;
    use super::*;
    use crate::cell::{CellStyle, SheetCell};
    use crate::workbook::SheetData;

    fn workbook_with(cells: &[((u32, u32), &str)]) -> WorkbookModel {
        let mut workbook = WorkbookModel::new_default();
        let mut sheet = SheetData::new("sheet-1", "Sheet1");
        for ((row, col), raw) in cells {
            sheet.cells.insert(
                format!("{}:{}", row, col),
                SheetCell {
                    raw_value: raw.to_string(),
                    display_value: raw.to_string(),
                    formula: None,
                    style: None,
                },
            );
        }
        workbook.sheets[0] = sheet;
        workbook
    }

    #[test]
    fn detects_linear_step_from_two_seed_cells() {
        let mut workbook = workbook_with(&[((1, 1), "5"), ((2, 1), "10")]);
        workbook.fill_series(0, 2, 1, 5, 1);
        assert_eq!(workbook.sheets[0].cells["3:1"].raw_value, "15");
        assert_eq!(workbook.sheets[0].cells["4:1"].raw_value, "20");
        assert_eq!(workbook.sheets[0].cells["5:1"].raw_value, "25");
    }

    #[test]
    fn header_above_seed_is_not_treated_as_step() {
        let mut workbook = workbook_with(&[((1, 1), "Qty"), ((2, 1), "3"), ((3, 1), "5")]);
        workbook.fill_series(0, 3, 1, 5, 1);
        assert_eq!(workbook.sheets[0].cells["4:1"].raw_value, "7");
        assert_eq!(workbook.sheets[0].cells["5:1"].raw_value, "9");
    }

    #[test]
    fn single_number_seed_still_steps_by_one() {
        let mut workbook = workbook_with(&[((1, 1), "3")]);
        workbook.fill_series(0, 1, 1, 3, 1);
        assert_eq!(workbook.sheets[0].cells["2:1"].raw_value, "4");
        assert_eq!(workbook.sheets[0].cells["3:1"].raw_value, "5");
    }

    #[test]
    fn text_seed_copies_verbatim() {
        let mut workbook = workbook_with(&[((1, 1), "Widget A"), ((2, 1), "Widget B")]);
        workbook.fill_series(0, 2, 1, 4, 1);
        assert_eq!(workbook.sheets[0].cells["3:1"].raw_value, "Widget B");
        assert_eq!(workbook.sheets[0].cells["4:1"].raw_value, "Widget B");
    }

    #[test]
    fn text_number_series_increments_suffix() {
        let mut workbook = workbook_with(&[((1, 1), "Item1"), ((2, 1), "Item2")]);
        workbook.fill_series(0, 2, 1, 4, 1);
        assert_eq!(workbook.sheets[0].cells["3:1"].raw_value, "Item3");
        assert_eq!(workbook.sheets[0].cells["4:1"].raw_value, "Item4");
    }

    #[test]
    fn single_text_number_seed_increments_by_one() {
        let mut workbook = workbook_with(&[((1, 1), "Item5")]);
        workbook.fill_series(0, 1, 1, 3, 1);
        assert_eq!(workbook.sheets[0].cells["2:1"].raw_value, "Item6");
        assert_eq!(workbook.sheets[0].cells["3:1"].raw_value, "Item7");
    }

    fn dated_cell(raw: &str) -> SheetCell {
        SheetCell {
            raw_value: raw.to_string(),
            display_value: raw.to_string(),
            formula: None,
            style: Some(CellStyle {
                format: Some("date".to_string()),
                ..Default::default()
            }),
        }
    }

    #[test]
    fn date_series_steps_days_for_date_formatted_serials() {
        let mut workbook = WorkbookModel::new_default();
        let mut sheet = SheetData::new("sheet-1", "Sheet1");
        sheet.cells.insert("1:1".to_string(), dated_cell("45138")); // 2023-09-01
        sheet.cells.insert("2:1".to_string(), dated_cell("45139"));
        workbook.sheets[0] = sheet;
        workbook.fill_series(0, 2, 1, 4, 1);
        assert_eq!(workbook.sheets[0].cells["3:1"].raw_value, "45140");
        assert_eq!(workbook.sheets[0].cells["4:1"].raw_value, "45141");
    }

    #[test]
    fn date_series_detects_month_steps_and_clamps() {
        let mut workbook = WorkbookModel::new_default();
        let mut sheet = SheetData::new("sheet-1", "Sheet1");
        sheet.cells.insert("1:1".to_string(), dated_cell("45322")); // 2024-01-31
        sheet.cells.insert("2:1".to_string(), dated_cell("45351")); // 2024-02-29
        workbook.sheets[0] = sheet;
        workbook.fill_series(0, 2, 1, 3, 1);
        // Month step with Excel's end-of-month clamp: Feb 29 → Mar 29.
        let next = workbook.sheets[0].cells["3:1"]
            .raw_value
            .parse::<i64>()
            .unwrap();
        let (year, month, day) = serial_to_ymd(next);
        assert_eq!((year, month, day), (2024, 3, 29));
    }

    #[test]
    fn iso_text_dates_fill_as_iso_text() {
        let mut workbook = workbook_with(&[((1, 1), "2024-03-05"), ((2, 1), "2024-03-06")]);
        workbook.fill_series(0, 2, 1, 4, 1);
        assert_eq!(workbook.sheets[0].cells["3:1"].raw_value, "2024-03-07");
        assert_eq!(workbook.sheets[0].cells["4:1"].raw_value, "2024-03-08");
    }

    #[test]
    fn horizontal_fill_uses_row_seed() {
        let mut workbook = workbook_with(&[((1, 1), "2"), ((1, 2), "4")]);
        workbook.fill_series(0, 1, 2, 1, 4);
        assert_eq!(workbook.sheets[0].cells["1:3"].raw_value, "6");
        assert_eq!(workbook.sheets[0].cells["1:4"].raw_value, "8");
    }

    #[test]
    fn downward_fill_from_top_cell_overwrites_with_step_one() {
        let mut workbook = workbook_with(&[((2, 1), "10"), ((1, 1), "7")]);
        // Seed walk stops at the grid top: single-cell seed 7 extends by +1.
        workbook.fill_series(0, 1, 1, 5, 1);
        assert_eq!(workbook.sheets[0].cells["2:1"].raw_value, "8");
        assert_eq!(workbook.sheets[0].cells["3:1"].raw_value, "9");
        assert_eq!(workbook.sheets[0].cells["4:1"].raw_value, "10");
        assert_eq!(workbook.sheets[0].cells["5:1"].raw_value, "11");
    }

    #[test]
    fn upward_fill_steps_backwards() {
        let mut workbook = workbook_with(&[((2, 1), "7"), ((3, 1), "10")]);
        workbook.fill_series(0, 3, 1, 1, 1);
        // Seed (7, 10) step 3; dragging up extrapolates backwards from 10.
        assert_eq!(workbook.sheets[0].cells["2:1"].raw_value, "7");
        assert_eq!(workbook.sheets[0].cells["1:1"].raw_value, "4");
    }

    #[test]
    fn sort_range_is_unchanged() {
        let mut workbook = workbook_with(&[((1, 2), "B"), ((2, 2), "A")]);
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
    fn hide_flags_round_trip_through_serialization() {
        let mut workbook = workbook_with(&[((1, 1), "x")]);
        workbook.set_row_hidden(0, &[2, 3], true);
        workbook.set_col_hidden(0, &[5], true);
        let json = serde_json::to_string(&workbook).expect("serialize workbook");
        let restored: WorkbookModel = serde_json::from_str(&json).expect("deserialize workbook");
        assert!(restored.sheets[0].hidden_rows.contains(&2));
        assert!(restored.sheets[0].hidden_rows.contains(&3));
        assert!(restored.sheets[0].hidden_cols.contains(&5));

        let mut workbook = restored;
        workbook.set_row_hidden(0, &[2], false);
        assert!(!workbook.sheets[0].hidden_rows.contains(&2));
        assert!(workbook.sheets[0].hidden_rows.contains(&3));
        assert!(!workbook.sheets[0].hidden_rows.contains(&0));
    }
}
