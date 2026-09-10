use crate::ast::*;
use crate::functions::{eval_func, MAX_DYNAMIC_ARRAY_CELLS};
use crate::parser::parse_a1_range;
use std::collections::{HashMap, HashSet};

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

fn finite_number(value: f64) -> FormulaValue {
    if value.is_finite() {
        FormulaValue::Number(value)
    } else {
        // Overflow / NaN is Excel's #NUM! domain error.
        FormulaValue::Error(FormulaError::Num)
    }
}

fn range_cell_count(start_row: u32, start_col: u32, end_row: u32, end_col: u32) -> Option<usize> {
    let rows = u64::from(end_row.saturating_sub(start_row).saturating_add(1));
    let cols = u64::from(end_col.saturating_sub(start_col).saturating_add(1));
    usize::try_from(rows.checked_mul(cols)?).ok()
}

fn materialize_range<P: CellProvider>(
    provider: &P,
    sheet: Option<&str>,
    start_row: u32,
    start_col: u32,
    end_row: u32,
    end_col: u32,
) -> FormulaValue {
    let (start_row, start_col, end_row, end_col) =
        normalize_range_bounds(start_row, start_col, end_row, end_col);
    let Some(cell_count) = range_cell_count(start_row, start_col, end_row, end_col) else {
        return FormulaValue::Error(FormulaError::Value);
    };
    if cell_count == 0 || cell_count > MAX_DYNAMIC_ARRAY_CELLS {
        return FormulaValue::Error(FormulaError::Value);
    }
    let height = end_row - start_row + 1;
    let width = end_col - start_col + 1;
    let mut data = Vec::with_capacity(cell_count);
    for r in start_row..=end_row {
        for c in start_col..=end_col {
            data.push(provider.get_cell_value(sheet, r, c));
        }
    }
    FormulaValue::Array(data, height, width)
}

fn as_i32_offset(value: f64) -> Option<i32> {
    if !value.is_finite() {
        return None;
    }
    let truncated = value.trunc();
    if truncated > f64::from(i32::MAX) || truncated < f64::from(i32::MIN) {
        return None;
    }
    Some(truncated as i32)
}

fn as_positive_dimension(value: f64) -> Option<u32> {
    if !value.is_finite() || value < 1.0 {
        return None;
    }
    if value > MAX_DYNAMIC_ARRAY_CELLS as f64 {
        return None;
    }
    Some(value as u32)
}

pub trait CellProvider {
    fn get_cell_value(&self, sheet: Option<&str>, row: u32, col: u32) -> FormulaValue;
}

pub fn extract_dependencies(expr: &Expr) -> HashSet<(u32, u32)> {
    let mut deps = HashSet::new();

    fn collect(expr: &Expr, deps: &mut HashSet<(u32, u32)>) {
        match expr {
            Expr::CellRef { row, col, .. } => {
                deps.insert((*row, *col));
            }
            Expr::RangeRef {
                start_row,
                start_col,
                end_row,
                end_col,
                ..
            } => {
                let (start_row, start_col, end_row, end_col) =
                    normalize_range_bounds(*start_row, *start_col, *end_row, *end_col);
                let Some(cell_count) = range_cell_count(start_row, start_col, end_row, end_col)
                else {
                    return;
                };
                if cell_count > MAX_DYNAMIC_ARRAY_CELLS {
                    return;
                }
                for r in start_row..=end_row {
                    for c in start_col..=end_col {
                        deps.insert((r, c));
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
            _ => {}
        }
    }

    collect(expr, &mut deps);
    deps
}

pub fn eval_expr<P: CellProvider>(expr: &Expr, provider: &P) -> FormulaValue {
    eval_expr_with_env(expr, provider, &HashMap::new())
}

fn eval_expr_with_env<P: CellProvider>(
    expr: &Expr,
    provider: &P,
    env: &HashMap<String, FormulaValue>,
) -> FormulaValue {
    match expr {
        Expr::Literal(val) => val.clone(),
        Expr::CellRef { sheet, row, col } => provider.get_cell_value(sheet.as_deref(), *row, *col),
        Expr::RangeRef {
            sheet,
            start_row,
            start_col,
            end_row,
            end_col,
        } => materialize_range(
            provider,
            sheet.as_deref(),
            *start_row,
            *start_col,
            *end_row,
            *end_col,
        ),
        Expr::Name(name) => env
            .get(&name.to_ascii_uppercase())
            .cloned()
            .unwrap_or(FormulaValue::Error(FormulaError::Name)),
        Expr::Binary { left, op, right } => {
            let l_val = eval_expr_with_env(left, provider, env);
            let r_val = eval_expr_with_env(right, provider, env);
            eval_binary_op(&l_val, op, &r_val)
        }
        Expr::FunctionCall { name, args } => match name.as_str() {
            "LET" => eval_let(args, provider, env),
            "OFFSET" => eval_offset(args, provider, env),
            "INDIRECT" => {
                let mut evaled_args = Vec::new();
                for arg in args {
                    evaled_args.push(eval_expr_with_env(arg, provider, env));
                }
                eval_indirect(&evaled_args, provider)
            }
            "ROWS" | "COLUMNS" => eval_rows_or_columns(name, args, provider, env),
            _ => {
                let mut evaled_args = Vec::new();
                for arg in args {
                    evaled_args.push(eval_expr_with_env(arg, provider, env));
                }
                eval_func(name, &evaled_args)
            }
        },
    }
}

fn eval_let<P: CellProvider>(
    args: &[Expr],
    provider: &P,
    parent_env: &HashMap<String, FormulaValue>,
) -> FormulaValue {
    // LET takes one or more name/value pairs followed by a result expression.
    if args.len() < 3 || !(args.len() - 1).is_multiple_of(2) || args.len() > 129 {
        return FormulaValue::Error(FormulaError::Value);
    }
    let mut env = parent_env.clone();
    for pair_start in (0..args.len() - 1).step_by(2) {
        let name = match &args[pair_start] {
            Expr::Name(name) if !name.is_empty() => name.to_ascii_uppercase(),
            _ => return FormulaValue::Error(FormulaError::Name),
        };
        let value = eval_expr_with_env(&args[pair_start + 1], provider, &env);
        env.insert(name, value);
    }
    eval_expr_with_env(
        args.last().expect("LET has a result expression"),
        provider,
        &env,
    )
}

fn eval_binary_op(left: &FormulaValue, op: &BinaryOp, right: &FormulaValue) -> FormulaValue {
    if let FormulaValue::Error(e) = left {
        return FormulaValue::Error(e.clone());
    }
    if let FormulaValue::Error(e) = right {
        return FormulaValue::Error(e.clone());
    }

    match op {
        BinaryOp::Add => match (left, right) {
            (FormulaValue::Number(l), FormulaValue::Number(r)) => finite_number(l + r),
            _ => FormulaValue::Error(FormulaError::Value),
        },
        BinaryOp::Sub => match (left, right) {
            (FormulaValue::Number(l), FormulaValue::Number(r)) => finite_number(l - r),
            _ => FormulaValue::Error(FormulaError::Value),
        },
        BinaryOp::Mul => match (left, right) {
            (FormulaValue::Number(l), FormulaValue::Number(r)) => finite_number(l * r),
            _ => FormulaValue::Error(FormulaError::Value),
        },
        BinaryOp::Div => match (left, right) {
            (FormulaValue::Number(_), FormulaValue::Number(r)) if *r == 0.0 => {
                FormulaValue::Error(FormulaError::DivZero)
            }
            (FormulaValue::Number(l), FormulaValue::Number(r)) => finite_number(l / r),
            _ => FormulaValue::Error(FormulaError::Value),
        },
        BinaryOp::Concat => {
            let l_str = match left {
                FormulaValue::String(s) => s.clone(),
                FormulaValue::Number(n) => n.to_string(),
                _ => "".to_string(),
            };
            let r_str = match right {
                FormulaValue::String(s) => s.clone(),
                FormulaValue::Number(n) => n.to_string(),
                _ => "".to_string(),
            };
            FormulaValue::String(format!("{}{}", l_str, r_str))
        }
        BinaryOp::Eq => FormulaValue::Boolean(left == right),
        BinaryOp::Neq => FormulaValue::Boolean(left != right),
        BinaryOp::Lt => match (left, right) {
            (FormulaValue::Number(l), FormulaValue::Number(r)) => FormulaValue::Boolean(l < r),
            _ => FormulaValue::Boolean(false),
        },
        BinaryOp::Lte => match (left, right) {
            (FormulaValue::Number(l), FormulaValue::Number(r)) => FormulaValue::Boolean(l <= r),
            _ => FormulaValue::Boolean(false),
        },
        BinaryOp::Gt => match (left, right) {
            (FormulaValue::Number(l), FormulaValue::Number(r)) => FormulaValue::Boolean(l > r),
            _ => FormulaValue::Boolean(false),
        },
        BinaryOp::Gte => match (left, right) {
            (FormulaValue::Number(l), FormulaValue::Number(r)) => FormulaValue::Boolean(l >= r),
            _ => FormulaValue::Boolean(false),
        },
    }
}

fn eval_rows_or_columns<P: CellProvider>(
    name: &str,
    args: &[Expr],
    provider: &P,
    env: &HashMap<String, FormulaValue>,
) -> FormulaValue {
    let Some(arg) = args.first() else {
        return FormulaValue::Error(FormulaError::Value);
    };
    let dim = match arg {
        Expr::CellRef { .. } => 1,
        Expr::RangeRef {
            start_row,
            start_col,
            end_row,
            end_col,
            ..
        } => {
            let (start_row, start_col, end_row, end_col) =
                normalize_range_bounds(*start_row, *start_col, *end_row, *end_col);
            if name == "ROWS" {
                end_row.saturating_sub(start_row).saturating_add(1)
            } else {
                end_col.saturating_sub(start_col).saturating_add(1)
            }
        }
        other => {
            let evaled = eval_expr_with_env(other, provider, env);
            return eval_func(name, std::slice::from_ref(&evaled));
        }
    };
    FormulaValue::Number(f64::from(dim))
}

fn eval_offset<P: CellProvider>(
    args: &[Expr],
    provider: &P,
    env: &HashMap<String, FormulaValue>,
) -> FormulaValue {
    if args.len() < 3 {
        return FormulaValue::Error(FormulaError::Value);
    }

    let (sheet, start_row, start_col, end_row, end_col) = match &args[0] {
        Expr::CellRef { sheet, row, col } => (sheet.clone(), *row, *col, *row, *col),
        Expr::RangeRef {
            sheet,
            start_row,
            start_col,
            end_row,
            end_col,
        } => (sheet.clone(), *start_row, *start_col, *end_row, *end_col),
        _ => return FormulaValue::Error(FormulaError::Value),
    };

    let row_offset = match eval_expr_with_env(&args[1], provider, env) {
        FormulaValue::Number(n) => match as_i32_offset(n) {
            Some(offset) => offset,
            None => return FormulaValue::Error(FormulaError::Ref),
        },
        FormulaValue::Error(e) => return FormulaValue::Error(e),
        _ => return FormulaValue::Error(FormulaError::Value),
    };
    let col_offset = match eval_expr_with_env(&args[2], provider, env) {
        FormulaValue::Number(n) => match as_i32_offset(n) {
            Some(offset) => offset,
            None => return FormulaValue::Error(FormulaError::Ref),
        },
        FormulaValue::Error(e) => return FormulaValue::Error(e),
        _ => return FormulaValue::Error(FormulaError::Value),
    };

    let (start_row, start_col, end_row, end_col) =
        normalize_range_bounds(start_row, start_col, end_row, end_col);
    let base_height = end_row.saturating_sub(start_row) + 1;
    let base_width = end_col.saturating_sub(start_col) + 1;

    let height = if let Some(arg) = args.get(3) {
        match eval_expr_with_env(arg, provider, env) {
            FormulaValue::Number(n) => match as_positive_dimension(n) {
                Some(height) => height,
                None => return FormulaValue::Error(FormulaError::Value),
            },
            FormulaValue::Error(e) => return FormulaValue::Error(e),
            _ => return FormulaValue::Error(FormulaError::Value),
        }
    } else {
        base_height
    };

    let width = if let Some(arg) = args.get(4) {
        match eval_expr_with_env(arg, provider, env) {
            FormulaValue::Number(n) => match as_positive_dimension(n) {
                Some(width) => width,
                None => return FormulaValue::Error(FormulaError::Value),
            },
            FormulaValue::Error(e) => return FormulaValue::Error(e),
            _ => return FormulaValue::Error(FormulaError::Value),
        }
    } else {
        base_width
    };

    let new_start_row = if row_offset >= 0 {
        match start_row.checked_add(row_offset as u32) {
            Some(row) => row,
            None => return FormulaValue::Error(FormulaError::Ref),
        }
    } else {
        start_row.saturating_sub((-row_offset) as u32)
    };
    let new_start_col = if col_offset >= 0 {
        match start_col.checked_add(col_offset as u32) {
            Some(col) => col,
            None => return FormulaValue::Error(FormulaError::Ref),
        }
    } else {
        start_col.saturating_sub((-col_offset) as u32)
    };

    if new_start_row == 0 || new_start_col == 0 {
        return FormulaValue::Error(FormulaError::Ref);
    }

    let Some(new_end_row) = new_start_row.checked_add(height.saturating_sub(1)) else {
        return FormulaValue::Error(FormulaError::Ref);
    };
    let Some(new_end_col) = new_start_col.checked_add(width.saturating_sub(1)) else {
        return FormulaValue::Error(FormulaError::Ref);
    };

    let materialized = materialize_range(
        provider,
        sheet.as_deref(),
        new_start_row,
        new_start_col,
        new_end_row,
        new_end_col,
    );
    if height == 1 && width == 1 {
        return match materialized {
            FormulaValue::Array(data, _, _) => data.first().cloned().unwrap_or(FormulaValue::Empty),
            other => other,
        };
    }
    materialized
}

fn eval_indirect<P: CellProvider>(args: &[FormulaValue], provider: &P) -> FormulaValue {
    if args.is_empty() {
        return FormulaValue::Error(FormulaError::Value);
    }
    let ref_text = match &args[0] {
        FormulaValue::String(s) => s.clone(),
        FormulaValue::Error(e) => return FormulaValue::Error(e.clone()),
        _ => return FormulaValue::Error(FormulaError::Value),
    };

    let Some((sheet, start_row, start_col, end_row, end_col)) = parse_a1_range(&ref_text) else {
        return FormulaValue::Error(FormulaError::Ref);
    };

    let (start_row, start_col, end_row, end_col) =
        normalize_range_bounds(start_row, start_col, end_row, end_col);
    if start_row == end_row && start_col == end_col {
        return provider.get_cell_value(sheet.as_deref(), start_row, start_col);
    }

    materialize_range(
        provider,
        sheet.as_deref(),
        start_row,
        start_col,
        end_row,
        end_col,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_formula;
    use std::collections::HashMap;

    struct MapProvider {
        values: HashMap<(u32, u32), FormulaValue>,
    }

    impl CellProvider for MapProvider {
        fn get_cell_value(&self, _sheet: Option<&str>, row: u32, col: u32) -> FormulaValue {
            self.values
                .get(&(row, col))
                .cloned()
                .unwrap_or(FormulaValue::Empty)
        }
    }

    #[test]
    fn inverted_range_evaluates_same_as_normalized_range() {
        let provider = MapProvider {
            values: HashMap::from([
                ((1, 1), FormulaValue::Number(1.0)),
                ((1, 2), FormulaValue::Number(2.0)),
                ((2, 1), FormulaValue::Number(3.0)),
                ((2, 2), FormulaValue::Number(4.0)),
            ]),
        };

        let inverted = parse_formula("=SUM(B2:A1)").expect("inverted range parses");
        let normal = parse_formula("=SUM(A1:B2)").expect("normal range parses");

        assert_eq!(
            eval_expr(&inverted, &provider),
            eval_expr(&normal, &provider)
        );
        assert_eq!(eval_expr(&inverted, &provider), FormulaValue::Number(10.0));
    }

    #[test]
    fn arithmetic_overflow_is_reported_as_a_formula_error() {
        let provider = MapProvider {
            values: HashMap::new(),
        };
        let overflow = Expr::Binary {
            left: Box::new(Expr::Literal(FormulaValue::Number(f64::MAX))),
            op: BinaryOp::Mul,
            right: Box::new(Expr::Literal(FormulaValue::Number(2.0))),
        };
        assert_eq!(
            eval_expr(&overflow, &provider),
            FormulaValue::Error(FormulaError::Num)
        );

        let nan = Expr::Binary {
            left: Box::new(Expr::Literal(FormulaValue::Number(f64::NAN))),
            op: BinaryOp::Add,
            right: Box::new(Expr::Literal(FormulaValue::Number(1.0))),
        };
        assert_eq!(
            eval_expr(&nan, &provider),
            FormulaValue::Error(FormulaError::Num)
        );
    }

    #[test]
    fn indirect_with_empty_args_returns_value_error() {
        let provider = MapProvider {
            values: HashMap::new(),
        };
        let expr = parse_formula("=INDIRECT()").expect("indirect parses");
        assert_eq!(
            eval_expr(&expr, &provider),
            FormulaValue::Error(FormulaError::Value)
        );
    }

    #[test]
    fn offset_with_too_few_args_returns_value_error() {
        let provider = MapProvider {
            values: HashMap::from([((1, 1), FormulaValue::Number(5.0))]),
        };
        let expr = parse_formula("=OFFSET(A1)").expect("offset parses");
        assert_eq!(
            eval_expr(&expr, &provider),
            FormulaValue::Error(FormulaError::Value)
        );
    }

    #[test]
    fn let_bindings_are_scoped_and_sequential() {
        let provider = MapProvider {
            values: HashMap::new(),
        };
        let expr = parse_formula("=LET(x,2,LET(x,3,x)+x)").expect("LET parses");
        assert_eq!(eval_expr(&expr, &provider), FormulaValue::Number(5.0));
    }

    #[test]
    fn let_rejects_non_name_bindings() {
        let provider = MapProvider {
            values: HashMap::new(),
        };
        let expr = parse_formula("=LET(1,2,3)").expect("LET parses");
        assert_eq!(
            eval_expr(&expr, &provider),
            FormulaValue::Error(FormulaError::Name)
        );
    }

    #[test]
    fn let_can_bind_formula_errors_for_iferror() {
        let provider = MapProvider {
            values: HashMap::new(),
        };
        let expr = parse_formula("=LET(x,1/0,IFERROR(x,42))").expect("LET parses");
        assert_eq!(eval_expr(&expr, &provider), FormulaValue::Number(42.0));
    }

    #[test]
    fn indirect_normalizes_inverted_ranges() {
        let provider = MapProvider {
            values: HashMap::from([
                ((1, 1), FormulaValue::Number(1.0)),
                ((1, 2), FormulaValue::Number(2.0)),
                ((2, 1), FormulaValue::Number(3.0)),
                ((2, 2), FormulaValue::Number(4.0)),
            ]),
        };
        let inverted = parse_formula("=SUM(INDIRECT(\"B2:A1\"))").expect("indirect parses");
        let normal = parse_formula("=SUM(INDIRECT(\"A1:B2\"))").expect("indirect parses");
        assert_eq!(
            eval_expr(&inverted, &provider),
            eval_expr(&normal, &provider)
        );
        assert_eq!(eval_expr(&inverted, &provider), FormulaValue::Number(10.0));
    }

    #[test]
    fn oversized_range_and_offset_return_value_error() {
        let provider = MapProvider {
            values: HashMap::new(),
        };
        let huge = parse_formula("=SUM(A1:XFD1048576)").expect("huge range parses");
        assert_eq!(
            eval_expr(&huge, &provider),
            FormulaValue::Error(FormulaError::Value)
        );
        assert!(extract_dependencies(&huge).is_empty());
        let rows = parse_formula("=ROWS(A1:XFD1048576)").expect("rows parses");
        assert_eq!(
            eval_expr(&rows, &provider),
            FormulaValue::Number(1_048_576.0)
        );

        let offset = parse_formula("=OFFSET(A1,0,0,100001,1)").expect("offset parses");
        assert_eq!(
            eval_expr(&offset, &provider),
            FormulaValue::Error(FormulaError::Value)
        );
    }
}
