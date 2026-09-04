use crate::ast::*;
use crate::functions::eval_func;
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
        } => {
            let (start_row, start_col, end_row, end_col) =
                normalize_range_bounds(*start_row, *start_col, *end_row, *end_col);
            let mut data = Vec::new();
            for r in start_row..=end_row {
                for c in start_col..=end_col {
                    data.push(provider.get_cell_value(sheet.as_deref(), r, c));
                }
            }
            FormulaValue::Array(data, end_row - start_row + 1, end_col - start_col + 1)
        }
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
        FormulaValue::Number(n) => n as i32,
        FormulaValue::Error(e) => return FormulaValue::Error(e),
        _ => return FormulaValue::Error(FormulaError::Value),
    };
    let col_offset = match eval_expr_with_env(&args[2], provider, env) {
        FormulaValue::Number(n) => n as i32,
        FormulaValue::Error(e) => return FormulaValue::Error(e),
        _ => return FormulaValue::Error(FormulaError::Value),
    };

    let base_height = end_row.saturating_sub(start_row) + 1;
    let base_width = end_col.saturating_sub(start_col) + 1;

    let height = if let Some(arg) = args.get(3) {
        match eval_expr_with_env(arg, provider, env) {
            FormulaValue::Number(n) if n >= 1.0 => n as u32,
            FormulaValue::Error(e) => return FormulaValue::Error(e),
            _ => return FormulaValue::Error(FormulaError::Value),
        }
    } else {
        base_height
    };

    let width = if let Some(arg) = args.get(4) {
        match eval_expr_with_env(arg, provider, env) {
            FormulaValue::Number(n) if n >= 1.0 => n as u32,
            FormulaValue::Error(e) => return FormulaValue::Error(e),
            _ => return FormulaValue::Error(FormulaError::Value),
        }
    } else {
        base_width
    };

    let new_start_row = if row_offset >= 0 {
        start_row + row_offset as u32
    } else {
        start_row.saturating_sub((-row_offset) as u32)
    };
    let new_start_col = if col_offset >= 0 {
        start_col + col_offset as u32
    } else {
        start_col.saturating_sub((-col_offset) as u32)
    };

    if new_start_row == 0 || new_start_col == 0 {
        return FormulaValue::Error(FormulaError::Ref);
    }

    let new_end_row = new_start_row + height - 1;
    let new_end_col = new_start_col + width - 1;

    let mut data = Vec::new();
    for r in new_start_row..=new_end_row {
        for c in new_start_col..=new_end_col {
            data.push(provider.get_cell_value(sheet.as_deref(), r, c));
        }
    }

    if height == 1 && width == 1 {
        return data.first().cloned().unwrap_or(FormulaValue::Empty);
    }

    FormulaValue::Array(data, height, width)
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

    let parsed = parse_a1_range(&ref_text);
    if parsed.is_none() {
        return FormulaValue::Error(FormulaError::Ref);
    }
    let (sheet, start_row, start_col, end_row, end_col) = parsed.unwrap();

    if start_row == end_row && start_col == end_col {
        return provider.get_cell_value(sheet.as_deref(), start_row, start_col);
    }

    let height = end_row - start_row + 1;
    let width = end_col - start_col + 1;
    let mut data = Vec::new();
    for r in start_row..=end_row {
        for c in start_col..=end_col {
            data.push(provider.get_cell_value(sheet.as_deref(), r, c));
        }
    }
    FormulaValue::Array(data, height, width)
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
}
