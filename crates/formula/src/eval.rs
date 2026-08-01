use crate::ast::*;
use crate::functions::eval_func;
use std::collections::HashSet;

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
                for r in *start_row..=*end_row {
                    for c in *start_col..=*end_col {
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
            let mut data = Vec::new();
            for r in *start_row..=*end_row {
                for c in *start_col..=*end_col {
                    data.push(provider.get_cell_value(sheet.as_deref(), r, c));
                }
            }
            FormulaValue::Array(data, *end_row - *start_row + 1, *end_col - *start_col + 1)
        }
        Expr::Binary { left, op, right } => {
            let l_val = eval_expr(left, provider);
            let r_val = eval_expr(right, provider);
            eval_binary_op(&l_val, op, &r_val)
        }
        Expr::FunctionCall { name, args } => {
            let mut evaled_args = Vec::new();
            for arg in args {
                evaled_args.push(eval_expr(arg, provider));
            }
            eval_func(name, &evaled_args)
        }
    }
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
            (FormulaValue::Number(l), FormulaValue::Number(r)) => FormulaValue::Number(l + r),
            _ => FormulaValue::Error(FormulaError::Value),
        },
        BinaryOp::Sub => match (left, right) {
            (FormulaValue::Number(l), FormulaValue::Number(r)) => FormulaValue::Number(l - r),
            _ => FormulaValue::Error(FormulaError::Value),
        },
        BinaryOp::Mul => match (left, right) {
            (FormulaValue::Number(l), FormulaValue::Number(r)) => FormulaValue::Number(l * r),
            _ => FormulaValue::Error(FormulaError::Value),
        },
        BinaryOp::Div => match (left, right) {
            (FormulaValue::Number(_), FormulaValue::Number(r)) if *r == 0.0 => {
                FormulaValue::Error(FormulaError::DivZero)
            }
            (FormulaValue::Number(l), FormulaValue::Number(r)) => FormulaValue::Number(l / r),
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
