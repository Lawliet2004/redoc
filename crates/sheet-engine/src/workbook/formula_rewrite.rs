use super::model::NamedRange;
use redoc_formula::{tokenize, Expr, Token, MAX_DYNAMIC_ARRAY_CELLS};
use std::collections::HashSet;

fn normalize_range_bounds(
    start_row: u32,
    start_col: u32,
    end_row: u32,
    end_col: u32,
) -> (u32, u32, u32, u32) {
    (
        start_row.min(end_row),
        start_col.min(end_col),
        start_row.max(end_row),
        start_col.max(end_col),
    )
}

fn insert_range_cells(
    deps: &mut HashSet<(u32, u32)>,
    start_row: u32,
    start_col: u32,
    end_row: u32,
    end_col: u32,
) {
    let (start_row, start_col, end_row, end_col) =
        normalize_range_bounds(start_row, start_col, end_row, end_col);
    let rows = u64::from(end_row.saturating_sub(start_row).saturating_add(1));
    let cols = u64::from(end_col.saturating_sub(start_col).saturating_add(1));
    let Some(count) = rows.checked_mul(cols) else {
        return;
    };
    if count == 0 || count > MAX_DYNAMIC_ARRAY_CELLS as u64 {
        return;
    }
    for row in start_row..=end_row {
        for col in start_col..=end_col {
            deps.insert((row, col));
        }
    }
}

pub fn expand_named_ranges(formula: &str, named_ranges: &[NamedRange]) -> String {
    let has_eq = formula.starts_with('=');
    let text = if has_eq { &formula[1..] } else { formula };

    let mut result = String::new();
    if has_eq {
        result.push('=');
    }

    let let_binding_names = collect_let_binding_names(text);

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
                    if !let_binding_names.contains(&ident.to_ascii_uppercase()) && nr.name == ident
                    {
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

fn collect_let_binding_names(formula: &str) -> HashSet<String> {
    let Ok(tokens) = tokenize(formula) else {
        return HashSet::new();
    };
    let mut names = HashSet::new();
    for (index, token) in tokens.iter().enumerate() {
        if token != &Token::Identifier("LET".to_string())
            || tokens.get(index + 1) != Some(&Token::LParen)
        {
            continue;
        }
        let mut depth = 1usize;
        let mut arg_index = 0usize;
        let mut at_arg_start = true;
        let mut candidates: Vec<(usize, String)> = Vec::new();
        for token in tokens.iter().skip(index + 2) {
            match token {
                Token::LParen => {
                    depth += 1;
                    at_arg_start = false;
                }
                Token::RParen => {
                    if depth == 1 {
                        let arg_count = arg_index + usize::from(!at_arg_start);
                        for (candidate_index, candidate) in candidates {
                            if candidate_index + 1 < arg_count {
                                names.insert(candidate.to_ascii_uppercase());
                            }
                        }
                        break;
                    }
                    depth -= 1;
                    at_arg_start = false;
                }
                Token::Comma if depth == 1 => {
                    arg_index += 1;
                    at_arg_start = true;
                }
                Token::Identifier(name) if depth == 1 && at_arg_start => {
                    candidates.push((arg_index, name.clone()));
                    at_arg_start = false;
                }
                _ => at_arg_start = false,
            }
        }
    }
    names
}
pub fn extract_local_dependencies(expr: &Expr) -> HashSet<(u32, u32)> {
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
            } => insert_range_cells(deps, *start_row, *start_col, *end_row, *end_col),
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
            Expr::Literal(_) | Expr::Name(_) => {}
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

pub fn extract_external_dependencies(
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
                    let mut local = HashSet::new();
                    insert_range_cells(&mut local, *start_row, *start_col, *end_row, *end_col);
                    for (row, col) in local {
                        deps.insert((target_sheet, row, col));
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
