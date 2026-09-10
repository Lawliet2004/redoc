use crate::ast::{FormulaError, FormulaValue};
use std::collections::{hash_map::DefaultHasher, HashSet};
use std::hash::{Hash, Hasher};
use std::thread;
use std::time::SystemTime;

const EXCEL_EPOCH_YEAR: i32 = 1899;
const EXCEL_EPOCH_MONTH: i32 = 12;
const EXCEL_EPOCH_DAY: i32 = 30;
// Keep serial-to-civil conversion bounded so malformed workbooks cannot
// overflow the day arithmetic or force unbounded date work.
const MAX_DATE_SERIAL: f64 = 10_000_000.0;

fn finite_result(value: f64) -> FormulaValue {
    if value.is_finite() {
        FormulaValue::Number(value)
    } else {
        // Overflow / NaN is Excel's #NUM! domain error.
        FormulaValue::Error(FormulaError::Num)
    }
}

fn flatten_args(args: &[FormulaValue]) -> Vec<FormulaValue> {
    let mut flat = Vec::new();
    for arg in args {
        match arg {
            FormulaValue::Array(data, _, _) => flat.extend(data.iter().cloned()),
            _ => flat.push(arg.clone()),
        }
    }
    flat
}

pub fn iter_args<'a>(args: &'a [FormulaValue]) -> impl Iterator<Item = &'a FormulaValue> + 'a {
    args.iter().flat_map(|arg| {
        let it: Box<dyn Iterator<Item = &'a FormulaValue>> = match arg {
            FormulaValue::Array(data, _, _) => Box::new(data.iter()),
            _ => Box::new(std::iter::once(arg)),
        };
        it
    })
}

pub fn eval_func(name: &str, args: &[FormulaValue]) -> FormulaValue {
    match name {
        "SUM" => {
            let mut sum = 0.0;
            for arg in iter_args(args) {
                if let FormulaValue::Number(n) = arg {
                    sum += n;
                } else if let FormulaValue::Error(e) = arg {
                    return FormulaValue::Error(e.clone());
                }
            }
            finite_result(sum)
        }
        "AVERAGE" => {
            let mut sum = 0.0;
            let mut count = 0;
            for arg in iter_args(args) {
                if let FormulaValue::Number(n) = arg {
                    sum += n;
                    count += 1;
                } else if let FormulaValue::Error(e) = arg {
                    return FormulaValue::Error(e.clone());
                }
            }
            if count == 0 {
                FormulaValue::Error(FormulaError::DivZero)
            } else {
                finite_result(sum / count as f64)
            }
        }
        "MIN" => {
            let mut min_val = f64::INFINITY;
            let mut found = false;
            for arg in iter_args(args) {
                if let FormulaValue::Number(n) = arg {
                    if !n.is_finite() {
                        return FormulaValue::Error(FormulaError::Value);
                    }
                    if *n < min_val {
                        min_val = *n;
                    }
                    found = true;
                } else if let FormulaValue::Error(e) = arg {
                    return FormulaValue::Error(e.clone());
                }
            }
            if found {
                FormulaValue::Number(min_val)
            } else {
                FormulaValue::Number(0.0)
            }
        }
        "MAX" => {
            let mut max_val = f64::NEG_INFINITY;
            let mut found = false;
            for arg in iter_args(args) {
                if let FormulaValue::Number(n) = arg {
                    if !n.is_finite() {
                        return FormulaValue::Error(FormulaError::Value);
                    }
                    if *n > max_val {
                        max_val = *n;
                    }
                    found = true;
                } else if let FormulaValue::Error(e) = arg {
                    return FormulaValue::Error(e.clone());
                }
            }
            if found {
                FormulaValue::Number(max_val)
            } else {
                FormulaValue::Number(0.0)
            }
        }
        "COUNT" => {
            let args_flat = iter_args(args);
            let count = args_flat
                .filter(|a| matches!(a, FormulaValue::Number(_)))
                .count();
            FormulaValue::Number(count as f64)
        }
        "COUNTA" => {
            let args_flat = iter_args(args);
            let count = args_flat
                .filter(|a| !matches!(a, FormulaValue::Empty))
                .count();
            FormulaValue::Number(count as f64)
        }
        "COUNTBLANK" => {
            let count = iter_args(args)
                .filter(|value| match value {
                    FormulaValue::Empty => true,
                    FormulaValue::String(text) => text.is_empty(),
                    _ => false,
                })
                .count();
            FormulaValue::Number(count as f64)
        }
        "IF" => {
            if args.is_empty() {
                return FormulaValue::Error(FormulaError::Value);
            }
            let cond = match &args[0] {
                FormulaValue::Boolean(b) => *b,
                FormulaValue::Number(n) => *n != 0.0,
                FormulaValue::Error(e) => return FormulaValue::Error(e.clone()),
                _ => false,
            };
            if cond {
                args.get(1).cloned().unwrap_or(FormulaValue::Boolean(true))
            } else {
                args.get(2).cloned().unwrap_or(FormulaValue::Boolean(false))
            }
        }
        "AND" => {
            for arg in iter_args(args) {
                match arg {
                    FormulaValue::Boolean(b) if !b => return FormulaValue::Boolean(false),
                    FormulaValue::Number(n) if *n == 0.0 => return FormulaValue::Boolean(false),
                    FormulaValue::Error(e) => return FormulaValue::Error(e.clone()),
                    _ => {}
                }
            }
            FormulaValue::Boolean(true)
        }
        "OR" => {
            for arg in iter_args(args) {
                match arg {
                    FormulaValue::Boolean(b) if *b => return FormulaValue::Boolean(true),
                    FormulaValue::Number(n) if *n != 0.0 => return FormulaValue::Boolean(true),
                    FormulaValue::Error(e) => return FormulaValue::Error(e.clone()),
                    _ => {}
                }
            }
            FormulaValue::Boolean(false)
        }
        "XOR" => {
            let mut truthy = 0usize;
            for arg in iter_args(args) {
                match arg {
                    FormulaValue::Boolean(value) if *value => truthy += 1,
                    FormulaValue::Number(value) if *value != 0.0 => truthy += 1,
                    FormulaValue::Error(error) => return FormulaValue::Error(error.clone()),
                    _ => {}
                }
            }
            FormulaValue::Boolean(truthy % 2 == 1)
        }
        "NOT" => {
            if args.is_empty() {
                return FormulaValue::Error(FormulaError::Value);
            }
            match &args[0] {
                FormulaValue::Boolean(b) => FormulaValue::Boolean(!b),
                FormulaValue::Number(n) => FormulaValue::Boolean(*n == 0.0),
                FormulaValue::Error(e) => FormulaValue::Error(e.clone()),
                _ => FormulaValue::Boolean(false),
            }
        }
        "ROUND" => {
            if args.is_empty() {
                return FormulaValue::Error(FormulaError::Value);
            }
            let n = match &args[0] {
                FormulaValue::Number(num) => *num,
                FormulaValue::Error(e) => return FormulaValue::Error(e.clone()),
                _ => return FormulaValue::Error(FormulaError::Value),
            };
            let digits = match args.get(1) {
                Some(FormulaValue::Number(d)) => (*d as i32).clamp(-15, 15),
                _ => 0,
            };
            let mult = 10f64.powi(digits);
            if mult == 0.0 {
                FormulaValue::Number(n)
            } else {
                finite_result((n * mult).round() / mult)
            }
        }
        "ROUNDUP" => eval_round_direction(args, true),
        "ROUNDDOWN" => eval_round_direction(args, false),
        "INT" => eval_int(args),
        "TRUNC" => eval_trunc(args),
        "SIGN" => eval_sign(args),
        "LN" => eval_logarithm(args, None),
        "LOG" => eval_logarithm(args, Some(10.0)),
        "LOG10" => eval_log10(args),
        "EXP" => eval_unary_math(args, f64::exp),
        "SIN" => eval_unary_math(args, f64::sin),
        "COS" => eval_unary_math(args, f64::cos),
        "TAN" => eval_unary_math(args, f64::tan),
        "CONCAT" | "CONCATENATE" => {
            let mut res = String::new();
            for arg in iter_args(args) {
                match arg {
                    FormulaValue::String(s) => res.push_str(s),
                    FormulaValue::Number(n) => res.push_str(&n.to_string()),
                    FormulaValue::Boolean(b) => res.push_str(&b.to_string()),
                    FormulaValue::Error(e) => return FormulaValue::Error(e.clone()),
                    _ => {}
                }
            }
            FormulaValue::String(res)
        }
        "LEFT" => {
            if args.is_empty() {
                return FormulaValue::Error(FormulaError::Value);
            }
            let s = match &args[0] {
                FormulaValue::String(str_val) => str_val.clone(),
                FormulaValue::Number(n) => n.to_string(),
                FormulaValue::Error(e) => return FormulaValue::Error(e.clone()),
                _ => return FormulaValue::Error(FormulaError::Value),
            };
            let len = match args.get(1) {
                Some(FormulaValue::Number(n)) => *n as usize,
                _ => 1,
            };
            let res: String = s.chars().take(len).collect();
            FormulaValue::String(res)
        }
        "RIGHT" => {
            if args.is_empty() {
                return FormulaValue::Error(FormulaError::Value);
            }
            let s = match &args[0] {
                FormulaValue::String(str_val) => str_val.clone(),
                FormulaValue::Number(n) => n.to_string(),
                FormulaValue::Error(e) => return FormulaValue::Error(e.clone()),
                _ => return FormulaValue::Error(FormulaError::Value),
            };
            let len = match args.get(1) {
                Some(FormulaValue::Number(n)) => *n as usize,
                _ => 1,
            };
            let total = s.chars().count();
            let skip = total.saturating_sub(len);
            let res: String = s.chars().skip(skip).collect();
            FormulaValue::String(res)
        }
        "LEN" => {
            if args.is_empty() {
                return FormulaValue::Error(FormulaError::Value);
            }
            let len = match &args[0] {
                FormulaValue::String(s) => s.chars().count(),
                FormulaValue::Number(n) => n.to_string().chars().count(),
                FormulaValue::Error(e) => return FormulaValue::Error(e.clone()),
                _ => 0,
            };
            FormulaValue::Number(len as f64)
        }
        "ABS" => {
            if args.is_empty() {
                return FormulaValue::Error(FormulaError::Value);
            }
            match &args[0] {
                FormulaValue::Number(n) => finite_result(n.abs()),
                FormulaValue::Error(e) => FormulaValue::Error(e.clone()),
                _ => FormulaValue::Error(FormulaError::Value),
            }
        }
        "COUNTIF" => {
            if args.len() < 2 {
                return FormulaValue::Error(FormulaError::Value);
            }
            let criterion = match &args[1] {
                FormulaValue::String(s) => s.clone(),
                FormulaValue::Number(n) => n.to_string(),
                FormulaValue::Boolean(b) => b.to_string(),
                FormulaValue::Error(e) => return FormulaValue::Error(e.clone()),
                FormulaValue::Empty => String::new(),
                FormulaValue::Array(..) => return FormulaValue::Error(FormulaError::Value),
            };
            let count = iter_args(&args[..1])
                .filter(|a| matches_criterion(a, &criterion))
                .count();
            FormulaValue::Number(count as f64)
        }
        "SUMIF" => {
            if args.len() < 2 {
                return FormulaValue::Error(FormulaError::Value);
            }
            let criterion = match &args[1] {
                FormulaValue::String(s) => s.clone(),
                FormulaValue::Number(n) => n.to_string(),
                FormulaValue::Boolean(b) => b.to_string(),
                FormulaValue::Error(e) => return FormulaValue::Error(e.clone()),
                FormulaValue::Empty => String::new(),
                FormulaValue::Array(..) => return FormulaValue::Error(FormulaError::Value),
            };
            let mut sum = 0.0;
            let range = iter_args(&args[0..1]);
            let sum_range: Box<dyn Iterator<Item = &FormulaValue>> = if args.len() >= 3 {
                Box::new(iter_args(&args[2..3]))
            } else {
                Box::new(iter_args(&args[0..1]))
            };
            for (val, sum_val) in range.zip(sum_range) {
                if matches_criterion(val, &criterion) {
                    if let FormulaValue::Number(n) = sum_val {
                        sum += n;
                    }
                }
            }
            FormulaValue::Number(sum)
        }
        "TODAY" | "NOW" => {
            // Excel serial date: days since 1899-12-30 (25569 = 1970-01-01).
            let secs = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs_f64())
                .unwrap_or(0.0);
            let serial = secs / 86400.0 + 25569.0;
            if name == "TODAY" {
                FormulaValue::Number(serial.floor())
            } else {
                FormulaValue::Number(serial)
            }
        }
        "TEXT" => {
            if args.is_empty() {
                return FormulaValue::Error(FormulaError::Value);
            }
            let value = match &args[0] {
                FormulaValue::Number(n) => *n,
                FormulaValue::String(s) => {
                    return FormulaValue::String(s.clone());
                }
                FormulaValue::Boolean(b) => {
                    return FormulaValue::String(b.to_string());
                }
                FormulaValue::Error(e) => return FormulaValue::Error(e.clone()),
                FormulaValue::Empty => return FormulaValue::String(String::new()),
                FormulaValue::Array(..) => return FormulaValue::Error(FormulaError::Value),
            };
            let format = match args.get(1) {
                Some(FormulaValue::String(s)) => s.as_str(),
                _ => "0",
            };
            let formatted = if format.contains('.') {
                let decimals = format
                    .split('.')
                    .nth(1)
                    .map(|part| part.chars().filter(|c| *c == '0' || *c == '#').count())
                    .unwrap_or(2);
                format!("{:.prec$}", value, prec = decimals)
            } else if format.contains('%') {
                format!("{:.0}%", value * 100.0)
            } else {
                format!("{:.0}", value)
            };
            FormulaValue::String(formatted)
        }
        "VLOOKUP" => {
            if args.len() < 3 {
                return FormulaValue::Error(FormulaError::Value);
            }
            let lookup = match &args[0] {
                FormulaValue::Array(data, _, _) => {
                    data.first().unwrap_or(&FormulaValue::Empty).clone()
                }
                v => v.clone(),
            };
            let col_index = match &args[2] {
                FormulaValue::Number(n) => *n as usize,
                _ => return FormulaValue::Error(FormulaError::Value),
            };
            if col_index < 1 {
                return FormulaValue::Error(FormulaError::Value);
            }
            let range_lookup = match args.get(3) {
                None => true,
                Some(FormulaValue::Boolean(b)) => *b,
                Some(FormulaValue::Number(n)) => *n != 0.0,
                Some(FormulaValue::Error(e)) => return FormulaValue::Error(e.clone()),
                _ => true,
            };
            let (table_data, width) = match &args[1] {
                FormulaValue::Array(data, _, cols) => (data.as_slice(), *cols as usize),
                val => (std::slice::from_ref(val), 1),
            };
            if table_data.is_empty() || width < col_index {
                return FormulaValue::Error(FormulaError::Ref);
            }
            if !range_lookup {
                for row in table_data.chunks(width) {
                    if values_equal(&row[0], &lookup) {
                        return row
                            .get(col_index - 1)
                            .cloned()
                            .unwrap_or(FormulaValue::Error(FormulaError::Ref));
                    }
                }
                return FormulaValue::Error(FormulaError::Na);
            }
            let mut last_match: Option<&[FormulaValue]> = None;
            for row in table_data.chunks(width) {
                if row.is_empty() {
                    continue;
                }
                if values_equal(&row[0], &lookup) {
                    return row
                        .get(col_index - 1)
                        .cloned()
                        .unwrap_or(FormulaValue::Error(FormulaError::Ref));
                }
                if vlookup_less_or_equal(&row[0], &lookup) {
                    last_match = Some(row);
                } else {
                    break;
                }
            }
            if let Some(row) = last_match {
                return row
                    .get(col_index - 1)
                    .cloned()
                    .unwrap_or(FormulaValue::Error(FormulaError::Ref));
            }
            FormulaValue::Error(FormulaError::Na)
        }
        "CEILING" => {
            if args.is_empty() {
                return FormulaValue::Error(FormulaError::Value);
            }
            let n = match &args[0] {
                FormulaValue::Number(num) => *num,
                FormulaValue::Error(e) => return FormulaValue::Error(e.clone()),
                _ => return FormulaValue::Error(FormulaError::Value),
            };
            let sig = match args.get(1) {
                Some(FormulaValue::Number(num)) => *num,
                Some(FormulaValue::Error(e)) => return FormulaValue::Error(e.clone()),
                None => 1.0,
                _ => return FormulaValue::Error(FormulaError::Value),
            };
            if sig == 0.0 {
                return FormulaValue::Number(0.0);
            }
            finite_result((n / sig).ceil() * sig)
        }
        "FLOOR" => {
            if args.is_empty() {
                return FormulaValue::Error(FormulaError::Value);
            }
            let n = match &args[0] {
                FormulaValue::Number(num) => *num,
                FormulaValue::Error(e) => return FormulaValue::Error(e.clone()),
                _ => return FormulaValue::Error(FormulaError::Value),
            };
            let sig = match args.get(1) {
                Some(FormulaValue::Number(num)) => *num,
                Some(FormulaValue::Error(e)) => return FormulaValue::Error(e.clone()),
                None => 1.0,
                _ => return FormulaValue::Error(FormulaError::Value),
            };
            if sig == 0.0 {
                return FormulaValue::Error(FormulaError::DivZero);
            }
            finite_result((n / sig).floor() * sig)
        }
        "MOD" => {
            if args.len() < 2 {
                return FormulaValue::Error(FormulaError::Value);
            }
            let n = match &args[0] {
                FormulaValue::Number(num) => *num,
                FormulaValue::Error(e) => return FormulaValue::Error(e.clone()),
                _ => return FormulaValue::Error(FormulaError::Value),
            };
            let d = match &args[1] {
                FormulaValue::Number(num) => *num,
                FormulaValue::Error(e) => return FormulaValue::Error(e.clone()),
                _ => return FormulaValue::Error(FormulaError::Value),
            };
            if d == 0.0 {
                return FormulaValue::Error(FormulaError::DivZero);
            }
            finite_result(n - d * (n / d).floor())
        }
        "POWER" => {
            if args.len() < 2 {
                return FormulaValue::Error(FormulaError::Value);
            }
            let n = match &args[0] {
                FormulaValue::Number(num) => *num,
                FormulaValue::Error(e) => return FormulaValue::Error(e.clone()),
                _ => return FormulaValue::Error(FormulaError::Value),
            };
            let p = match &args[1] {
                FormulaValue::Number(num) => *num,
                FormulaValue::Error(e) => return FormulaValue::Error(e.clone()),
                _ => return FormulaValue::Error(FormulaError::Value),
            };
            finite_result(n.powf(p))
        }
        "SQRT" => {
            if args.is_empty() {
                return FormulaValue::Error(FormulaError::Value);
            }
            let n = match &args[0] {
                FormulaValue::Number(num) => *num,
                FormulaValue::Error(e) => return FormulaValue::Error(e.clone()),
                _ => return FormulaValue::Error(FormulaError::Value),
            };
            if n < 0.0 {
                FormulaValue::Error(FormulaError::Value)
            } else {
                FormulaValue::Number(n.sqrt())
            }
        }
        "PRODUCT" => {
            let mut prod = 1.0;
            let mut found = false;
            for arg in iter_args(args) {
                if let FormulaValue::Number(n) = arg {
                    prod *= n;
                    found = true;
                } else if let FormulaValue::Error(e) = arg {
                    return FormulaValue::Error(e.clone());
                }
            }
            if found {
                finite_result(prod)
            } else {
                FormulaValue::Number(0.0)
            }
        }
        "UPPER" => {
            if args.is_empty() {
                return FormulaValue::Error(FormulaError::Value);
            }
            match &args[0] {
                FormulaValue::String(s) => FormulaValue::String(s.to_uppercase()),
                FormulaValue::Number(n) => FormulaValue::String(n.to_string()),
                FormulaValue::Boolean(b) => FormulaValue::String(b.to_string().to_uppercase()),
                FormulaValue::Error(e) => FormulaValue::Error(e.clone()),
                _ => FormulaValue::String(String::new()),
            }
        }
        "LOWER" => {
            if args.is_empty() {
                return FormulaValue::Error(FormulaError::Value);
            }
            match &args[0] {
                FormulaValue::String(s) => FormulaValue::String(s.to_lowercase()),
                FormulaValue::Number(n) => FormulaValue::String(n.to_string()),
                FormulaValue::Boolean(b) => FormulaValue::String(b.to_string().to_lowercase()),
                FormulaValue::Error(e) => FormulaValue::Error(e.clone()),
                _ => FormulaValue::String(String::new()),
            }
        }
        "PROPER" => {
            if args.is_empty() {
                return FormulaValue::Error(FormulaError::Value);
            }
            let s = match &args[0] {
                FormulaValue::String(s) => s.clone(),
                FormulaValue::Number(n) => n.to_string(),
                FormulaValue::Boolean(b) => b.to_string(),
                FormulaValue::Error(e) => return FormulaValue::Error(e.clone()),
                _ => return FormulaValue::String(String::new()),
            };
            let mut res = String::new();
            let mut capitalize_next = true;
            for c in s.chars() {
                if c.is_alphabetic() {
                    if capitalize_next {
                        res.push_str(&c.to_uppercase().to_string());
                        capitalize_next = false;
                    } else {
                        res.push_str(&c.to_lowercase().to_string());
                    }
                } else {
                    res.push(c);
                    capitalize_next = c.is_whitespace() || !c.is_alphanumeric();
                }
            }
            FormulaValue::String(res)
        }
        "MID" => {
            if args.len() < 3 {
                return FormulaValue::Error(FormulaError::Value);
            }
            let s = match &args[0] {
                FormulaValue::String(s) => s.clone(),
                FormulaValue::Number(n) => n.to_string(),
                FormulaValue::Error(e) => return FormulaValue::Error(e.clone()),
                _ => return FormulaValue::Error(FormulaError::Value),
            };
            let start = match &args[1] {
                FormulaValue::Number(n) => *n as usize,
                _ => return FormulaValue::Error(FormulaError::Value),
            };
            let len = match &args[2] {
                FormulaValue::Number(n) => *n as usize,
                _ => return FormulaValue::Error(FormulaError::Value),
            };
            if start < 1 {
                return FormulaValue::Error(FormulaError::Value);
            }
            let res: String = s.chars().skip(start - 1).take(len).collect();
            FormulaValue::String(res)
        }
        "TRIM" => {
            if args.is_empty() {
                return FormulaValue::Error(FormulaError::Value);
            }
            let s = match &args[0] {
                FormulaValue::String(s) => s.clone(),
                FormulaValue::Number(n) => n.to_string(),
                FormulaValue::Error(e) => return FormulaValue::Error(e.clone()),
                _ => return FormulaValue::Error(FormulaError::Value),
            };
            let parts: Vec<&str> = s.split_whitespace().collect();
            FormulaValue::String(parts.join(" "))
        }
        "IFERROR" => {
            if args.is_empty() {
                return FormulaValue::Error(FormulaError::Value);
            }
            match &args[0] {
                FormulaValue::Error(_) => args.get(1).cloned().unwrap_or(FormulaValue::Empty),
                val => val.clone(),
            }
        }
        "IFNA" => {
            if args.is_empty() {
                return FormulaValue::Error(FormulaError::Value);
            }
            match &args[0] {
                FormulaValue::Error(FormulaError::Na) => {
                    args.get(1).cloned().unwrap_or(FormulaValue::Empty)
                }
                val => val.clone(),
            }
        }
        "XLOOKUP" => eval_xlookup(args),
        "INDEX" => eval_index(args),
        "MATCH" => eval_match(args),
        "ROWS" => eval_rows(args),
        "COLUMNS" => eval_columns(args),
        "TRANSPOSE" => eval_transpose(args),
        "TOCOL" => eval_to_row_or_column(args, true),
        "TOROW" => eval_to_row_or_column(args, false),
        "WRAPROWS" => eval_wrap(args, true),
        "WRAPCOLS" => eval_wrap(args, false),
        "SEQUENCE" => eval_sequence(args),
        "FILTER" => eval_filter(args),
        "HSTACK" => eval_hstack(args),
        "TAKE" => eval_take_drop(args, true),
        "DROP" => eval_take_drop(args, false),
        "CHOOSECOLS" => eval_choose_axis(args, false),
        "CHOOSEROWS" => eval_choose_axis(args, true),
        "UNIQUE" => eval_unique(args),
        "SORT" => eval_sort(args),
        "SORTBY" => eval_sortby(args),
        "VSTACK" => eval_vstack(args),
        "SUMIFS" => eval_sumifs(args),
        "SUMPRODUCT" => eval_sumproduct(args),
        "COUNTIFS" => eval_countifs(args),
        "AVERAGEIF" => eval_averageif(args),
        "AVERAGEIFS" => eval_averageifs(args),
        "MAXIFS" => eval_maxifs(args),
        "MINIFS" => eval_minifs(args),
        "DATE" => eval_date(args),
        "DATEVALUE" => eval_datevalue(args),
        "TIME" => eval_time(args),
        "YEAR" => eval_year(args),
        "MONTH" => eval_month(args),
        "DAY" => eval_day(args),
        "HOUR" => eval_hour(args),
        "MINUTE" => eval_minute(args),
        "SECOND" => eval_second(args),
        "WEEKDAY" => eval_weekday(args),
        "NETWORKDAYS" => eval_networkdays(args),
        "DAYS" => eval_days(args),
        "EDATE" => eval_edate(args),
        "EOMONTH" => eval_eomonth(args),
        "WORKDAY" => eval_workday(args),
        "ISBLANK" => eval_isblank(args),
        "ISERROR" => eval_iserror(args),
        "ISNUMBER" => eval_isnumber(args),
        "ISTEXT" => eval_istext(args),
        "ISLOGICAL" => eval_islogical(args),
        "VALUE" => eval_value(args),
        "SUBSTITUTE" => eval_substitute(args),
        "REPLACE" => eval_replace(args),
        "FIND" => eval_find(args, true),
        "SEARCH" => eval_find(args, false),
        "RAND" => FormulaValue::Number(rand_unit()),
        "RANDARRAY" => eval_randarray(args),
        "RANDBETWEEN" => eval_randbetween(args),
        "HLOOKUP" => eval_hlookup(args),
        "CHOOSE" => eval_choose(args),
        "TRUE" => FormulaValue::Boolean(true),
        "FALSE" => FormulaValue::Boolean(false),
        "MEDIAN" => eval_median(args),
        "RANK" | "RANK.EQ" => eval_rank(args),
        "QUARTILE" | "QUARTILE.INC" => eval_quartile_inc(args),
        "CORREL" | "PEARSON" => eval_correl(args),
        "SUMSQ" => eval_sumsq(args),
        "STDEV.S" | "STDEV" => eval_stdev_sample(args),
        "STDEV.P" => eval_stdev_population(args),
        "VAR.S" | "VAR" => eval_var_sample(args),
        "VAR.P" => eval_var_population(args),
        "PERCENTILE.INC" | "PERCENTILE" => eval_percentile_inc(args),
        "PERCENTILE.EXC" => eval_percentile_exc(args),
        "TEXTJOIN" => eval_textjoin(args),
        "IFS" => eval_ifs(args),
        "SWITCH" => eval_switch(args),
        "FV" => eval_fv(args),
        "IRR" => eval_irr(args),
        "NPV" => eval_npv(args),
        "PMT" => eval_pmt(args),
        "PV" => eval_pv(args),
        _ => FormulaValue::Error(FormulaError::Name),
    }
}

fn eval_median(args: &[FormulaValue]) -> FormulaValue {
    let mut values: Vec<f64> = args
        .iter()
        .filter_map(|a| match a {
            FormulaValue::Number(n) => n.is_finite().then_some(*n),
            FormulaValue::Array(arr, _, _) => arr
                .iter()
                .filter_map(|v| match v {
                    FormulaValue::Number(n) => n.is_finite().then_some(*n),
                    _ => None,
                })
                .next(),
            _ => None,
        })
        .collect();
    if values.is_empty() {
        return FormulaValue::Error(FormulaError::DivZero);
    }
    values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mid = values.len() / 2;
    if values.len().is_multiple_of(2) {
        FormulaValue::Number((values[mid - 1] + values[mid]) / 2.0)
    } else {
        FormulaValue::Number(values[mid])
    }
}

fn rounding_args(args: &[FormulaValue]) -> Result<(f64, i32), FormulaError> {
    if !(1..=2).contains(&args.len()) {
        return Err(FormulaError::Value);
    }
    let number = match &args[0] {
        FormulaValue::Number(value) if value.is_finite() => *value,
        FormulaValue::Error(error) => return Err(error.clone()),
        _ => return Err(FormulaError::Value),
    };
    let digits = match args.get(1) {
        None => 0,
        Some(FormulaValue::Number(value)) if value.is_finite() => {
            value.trunc().clamp(-15.0, 15.0) as i32
        }
        Some(FormulaValue::Error(error)) => return Err(error.clone()),
        Some(_) => return Err(FormulaError::Value),
    };
    Ok((number, digits))
}

fn eval_round_direction(args: &[FormulaValue], away_from_zero: bool) -> FormulaValue {
    let (number, digits) = match rounding_args(args) {
        Ok(values) => values,
        Err(error) => return FormulaValue::Error(error),
    };
    let multiplier = 10f64.powi(digits);
    let scaled = number * multiplier;
    let rounded = if away_from_zero {
        if scaled.is_sign_negative() {
            scaled.floor()
        } else {
            scaled.ceil()
        }
    } else {
        scaled.trunc()
    };
    finite_result(rounded / multiplier)
}

fn eval_int(args: &[FormulaValue]) -> FormulaValue {
    if args.len() != 1 {
        return FormulaValue::Error(FormulaError::Value);
    }
    match &args[0] {
        FormulaValue::Number(value) if value.is_finite() => finite_result(value.floor()),
        FormulaValue::Error(error) => FormulaValue::Error(error.clone()),
        _ => FormulaValue::Error(FormulaError::Value),
    }
}

fn eval_trunc(args: &[FormulaValue]) -> FormulaValue {
    let (number, digits) = match rounding_args(args) {
        Ok(values) => values,
        Err(error) => return FormulaValue::Error(error),
    };
    finite_result((number * 10f64.powi(digits)).trunc() / 10f64.powi(digits))
}

fn eval_sign(args: &[FormulaValue]) -> FormulaValue {
    if args.len() != 1 {
        return FormulaValue::Error(FormulaError::Value);
    }
    match &args[0] {
        FormulaValue::Number(value) if value.is_finite() => FormulaValue::Number(if *value > 0.0 {
            1.0
        } else if *value < 0.0 {
            -1.0
        } else {
            0.0
        }),
        FormulaValue::Error(error) => FormulaValue::Error(error.clone()),
        _ => FormulaValue::Error(FormulaError::Value),
    }
}

fn eval_logarithm(args: &[FormulaValue], default_base: Option<f64>) -> FormulaValue {
    if args.len() != 1 && (default_base.is_none() || args.len() != 2) {
        return FormulaValue::Error(FormulaError::Value);
    }
    let number = match &args[0] {
        FormulaValue::Number(value) if value.is_finite() && *value > 0.0 => *value,
        FormulaValue::Error(error) => return FormulaValue::Error(error.clone()),
        _ => return FormulaValue::Error(FormulaError::Value),
    };
    let base = match default_base {
        None => std::f64::consts::E,
        Some(default) => match args.get(1) {
            None => default,
            Some(FormulaValue::Number(value)) if value.is_finite() => *value,
            Some(FormulaValue::Error(error)) => return FormulaValue::Error(error.clone()),
            Some(_) => return FormulaValue::Error(FormulaError::Value),
        },
    };
    if base.partial_cmp(&0.0) != Some(std::cmp::Ordering::Greater)
        || (base - 1.0).abs() < f64::EPSILON
    {
        return FormulaValue::Error(FormulaError::Value);
    }
    finite_result(number.log(base))
}

fn eval_log10(args: &[FormulaValue]) -> FormulaValue {
    if args.len() != 1 {
        return FormulaValue::Error(FormulaError::Value);
    }
    eval_logarithm(args, Some(10.0))
}

fn eval_unary_math(args: &[FormulaValue], operation: fn(f64) -> f64) -> FormulaValue {
    if args.len() != 1 {
        return FormulaValue::Error(FormulaError::Value);
    }
    let number = match &args[0] {
        FormulaValue::Number(value) if value.is_finite() => *value,
        FormulaValue::Error(error) => return FormulaValue::Error(error.clone()),
        _ => return FormulaValue::Error(FormulaError::Value),
    };
    finite_result(operation(number))
}

fn checked_numeric_values(value: &FormulaValue) -> Result<Vec<f64>, FormulaError> {
    let values = match value {
        FormulaValue::Array(data, _, _) => data,
        scalar => std::slice::from_ref(scalar),
    };
    let mut numbers = Vec::new();
    for value in values {
        match value {
            FormulaValue::Number(number) if number.is_finite() => numbers.push(*number),
            FormulaValue::Number(_) => return Err(FormulaError::Value),
            FormulaValue::Error(error) => return Err(error.clone()),
            FormulaValue::Boolean(_) | FormulaValue::Empty | FormulaValue::String(_) => {}
            FormulaValue::Array(..) => return Err(FormulaError::Value),
        }
    }
    Ok(numbers)
}

fn eval_rank(args: &[FormulaValue]) -> FormulaValue {
    if !(2..=3).contains(&args.len()) {
        return FormulaValue::Error(FormulaError::Value);
    }
    let number = match &args[0] {
        FormulaValue::Number(value) if value.is_finite() => *value,
        FormulaValue::Error(error) => return FormulaValue::Error(error.clone()),
        _ => return FormulaValue::Error(FormulaError::Value),
    };
    let values = match checked_numeric_values(&args[1]) {
        Ok(values) => values,
        Err(error) => return FormulaValue::Error(error),
    };
    if values.is_empty() {
        return FormulaValue::Error(FormulaError::Na);
    }
    let ascending = match args.get(2) {
        None => false,
        Some(FormulaValue::Number(value)) if value.is_finite() => *value != 0.0,
        Some(FormulaValue::Error(error)) => return FormulaValue::Error(error.clone()),
        Some(_) => return FormulaValue::Error(FormulaError::Value),
    };
    if !values.contains(&number) {
        return FormulaValue::Error(FormulaError::Na);
    }
    let rank = if ascending {
        values.iter().filter(|value| **value < number).count() + 1
    } else {
        values.iter().filter(|value| **value > number).count() + 1
    };
    FormulaValue::Number(rank as f64)
}

fn eval_quartile_inc(args: &[FormulaValue]) -> FormulaValue {
    if args.len() != 2 {
        return FormulaValue::Error(FormulaError::Value);
    }
    let quartile = match &args[1] {
        FormulaValue::Number(value) if value.is_finite() => *value as i32,
        FormulaValue::Error(error) => return FormulaValue::Error(error.clone()),
        _ => return FormulaValue::Error(FormulaError::Value),
    };
    if !(0..=4).contains(&quartile) {
        return FormulaValue::Error(FormulaError::DivZero);
    }
    let mut values = match checked_numeric_values(&args[0]) {
        Ok(values) => values,
        Err(error) => return FormulaValue::Error(error),
    };
    percentile_sorted(&mut values, quartile as f64 / 4.0)
        .map(finite_result)
        .unwrap_or(FormulaValue::Error(FormulaError::DivZero))
}

fn eval_correl(args: &[FormulaValue]) -> FormulaValue {
    if args.len() != 2 {
        return FormulaValue::Error(FormulaError::Value);
    }
    let x = match checked_numeric_values(&args[0]) {
        Ok(values) => values,
        Err(error) => return FormulaValue::Error(error),
    };
    let y = match checked_numeric_values(&args[1]) {
        Ok(values) => values,
        Err(error) => return FormulaValue::Error(error),
    };
    if x.len() != y.len() || x.len() < 2 {
        return FormulaValue::Error(FormulaError::Value);
    }
    let mean_x = x.iter().sum::<f64>() / x.len() as f64;
    let mean_y = y.iter().sum::<f64>() / y.len() as f64;
    let mut numerator = 0.0;
    let mut x_variance = 0.0;
    let mut y_variance = 0.0;
    for (x_value, y_value) in x.iter().zip(y.iter()) {
        let dx = *x_value - mean_x;
        let dy = *y_value - mean_y;
        numerator += dx * dy;
        x_variance += dx * dx;
        y_variance += dy * dy;
    }
    let denominator = (x_variance * y_variance).sqrt();
    if denominator == 0.0 {
        return FormulaValue::Error(FormulaError::DivZero);
    }
    finite_result(numerator / denominator)
}

fn eval_sumsq(args: &[FormulaValue]) -> FormulaValue {
    if args.is_empty() {
        return FormulaValue::Error(FormulaError::Value);
    }
    let mut total = 0.0;
    for arg in args {
        let values = match checked_numeric_values(arg) {
            Ok(values) => values,
            Err(error) => return FormulaValue::Error(error),
        };
        for value in values {
            total += value * value;
            if !total.is_finite() {
                return FormulaValue::Error(FormulaError::Value);
            }
        }
    }
    FormulaValue::Number(total)
}

fn numeric_values(args: &[FormulaValue]) -> Vec<f64> {
    let mut out: Vec<f64> = Vec::new();
    let take_into = |v: &FormulaValue, out: &mut Vec<f64>| {
        if let FormulaValue::Number(n) = v {
            if n.is_finite() {
                out.push(*n);
            }
        }
    };
    for arg in args {
        match arg {
            FormulaValue::Number(n) => {
                if n.is_finite() {
                    out.push(*n);
                }
            }
            FormulaValue::Array(arr, _, _) => {
                for v in arr {
                    take_into(v, &mut out);
                }
            }
            FormulaValue::Empty => {}
            other => take_into(other, &mut out),
        }
    }
    out
}

fn mean(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        None
    } else {
        Some(values.iter().sum::<f64>() / values.len() as f64)
    }
}

fn eval_stdev_sample(args: &[FormulaValue]) -> FormulaValue {
    let values = numeric_values(args);
    if values.len() < 2 {
        return FormulaValue::Error(FormulaError::DivZero);
    }
    let m = mean(&values).unwrap_or(0.0);
    let var: f64 =
        values.iter().map(|v| (v - m).powi(2)).sum::<f64>() / (values.len() as f64 - 1.0);
    FormulaValue::Number(var.sqrt())
}

fn eval_stdev_population(args: &[FormulaValue]) -> FormulaValue {
    let values = numeric_values(args);
    if values.is_empty() {
        return FormulaValue::Error(FormulaError::DivZero);
    }
    let m = mean(&values).unwrap_or(0.0);
    let var: f64 = values.iter().map(|v| (v - m).powi(2)).sum::<f64>() / values.len() as f64;
    FormulaValue::Number(var.sqrt())
}

fn eval_var_sample(args: &[FormulaValue]) -> FormulaValue {
    let values = numeric_values(args);
    if values.len() < 2 {
        return FormulaValue::Error(FormulaError::DivZero);
    }
    let m = mean(&values).unwrap_or(0.0);
    FormulaValue::Number(
        values.iter().map(|v| (v - m).powi(2)).sum::<f64>() / (values.len() as f64 - 1.0),
    )
}

fn eval_var_population(args: &[FormulaValue]) -> FormulaValue {
    let values = numeric_values(args);
    if values.is_empty() {
        return FormulaValue::Error(FormulaError::DivZero);
    }
    let m = mean(&values).unwrap_or(0.0);
    FormulaValue::Number(values.iter().map(|v| (v - m).powi(2)).sum::<f64>() / values.len() as f64)
}

fn percentile_sorted(values: &mut [f64], p: f64) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let rank = p * (values.len() as f64 - 1.0);
    let lo = rank.floor() as usize;
    let hi = rank.ceil() as usize;
    if lo == hi {
        Some(values[lo])
    } else {
        let frac = rank - lo as f64;
        Some(values[lo] * (1.0 - frac) + values[hi] * frac)
    }
}

fn eval_percentile_inc(args: &[FormulaValue]) -> FormulaValue {
    if args.len() != 2 {
        return FormulaValue::Error(FormulaError::Value);
    }
    let p = match &args[1] {
        FormulaValue::Number(n) => *n,
        _ => return FormulaValue::Error(FormulaError::Value),
    };
    if !(0.0..=1.0).contains(&p) {
        return FormulaValue::Error(FormulaError::DivZero);
    }
    let mut values = numeric_values(&[args[0].clone()]);
    match percentile_sorted(&mut values, p) {
        Some(v) => FormulaValue::Number(v),
        None => FormulaValue::Error(FormulaError::DivZero),
    }
}

fn eval_percentile_exc(args: &[FormulaValue]) -> FormulaValue {
    if args.len() != 2 {
        return FormulaValue::Error(FormulaError::Value);
    }
    let p = match &args[1] {
        FormulaValue::Number(n) => *n,
        _ => return FormulaValue::Error(FormulaError::Value),
    };
    if !(0.0..=1.0).contains(&p) {
        return FormulaValue::Error(FormulaError::DivZero);
    }
    let mut values = numeric_values(&[args[0].clone()]);
    if values.len() < 2 {
        return FormulaValue::Error(FormulaError::DivZero);
    }
    values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let rank = p * (values.len() as f64 + 1.0) - 1.0;
    if rank < 0.0 || rank > values.len() as f64 - 1.0 {
        return FormulaValue::Error(FormulaError::DivZero);
    }
    let lo = rank.floor() as usize;
    let hi = rank.ceil() as usize;
    if lo == hi {
        FormulaValue::Number(values[lo])
    } else {
        let frac = rank - lo as f64;
        FormulaValue::Number(values[lo] * (1.0 - frac) + values[hi] * frac)
    }
}

fn eval_textjoin(args: &[FormulaValue]) -> FormulaValue {
    if args.is_empty() {
        return FormulaValue::Error(FormulaError::Value);
    }
    let delimiter = match string_arg(&args[0]) {
        Some(s) => s,
        None => return FormulaValue::Error(FormulaError::Value),
    };
    let ignore_empty = matches!(&args[1], FormulaValue::Boolean(true));
    let mut out = String::new();
    let mut first = true;
    for chunk in &args[2..] {
        match chunk {
            FormulaValue::Array(arr, _, _) => {
                for v in arr {
                    if ignore_empty && matches!(v, FormulaValue::Empty) {
                        continue;
                    }
                    let s = string_arg(v).unwrap_or_default();
                    if ignore_empty && s.is_empty() {
                        continue;
                    }
                    if !first {
                        out.push_str(&delimiter);
                    }
                    out.push_str(&s);
                    first = false;
                }
            }
            FormulaValue::Empty if ignore_empty => continue,
            other => {
                let s = string_arg(other).unwrap_or_default();
                if ignore_empty && s.is_empty() {
                    continue;
                }
                if !first {
                    out.push_str(&delimiter);
                }
                out.push_str(&s);
                first = false;
            }
        }
    }
    FormulaValue::String(out)
}

fn eval_ifs(args: &[FormulaValue]) -> FormulaValue {
    if !args.len().is_multiple_of(2) || args.is_empty() {
        return FormulaValue::Error(FormulaError::Value);
    }
    let mut i = 0;
    while i + 1 < args.len() {
        let cond = match &args[i] {
            FormulaValue::Boolean(b) => *b,
            FormulaValue::Number(n) => *n != 0.0,
            _ => return FormulaValue::Error(FormulaError::Value),
        };
        if cond {
            return args[i + 1].clone();
        }
        i += 2;
    }
    FormulaValue::Error(FormulaError::Na)
}

fn eval_switch(args: &[FormulaValue]) -> FormulaValue {
    if args.len() < 3 {
        return FormulaValue::Error(FormulaError::Value);
    }
    let needle = &args[0];
    let mut i = 1;
    let mut default: Option<FormulaValue> = None;
    while i < args.len() {
        if i + 1 == args.len() {
            default = Some(args[i].clone());
            break;
        }
        if values_equal(needle, &args[i]) {
            return args[i + 1].clone();
        }
        i += 2;
    }
    default.unwrap_or(FormulaValue::Error(FormulaError::Na))
}

fn financial_number(
    args: &[FormulaValue],
    index: usize,
    default: Option<f64>,
) -> Result<f64, FormulaError> {
    match args.get(index) {
        Some(FormulaValue::Number(value)) if value.is_finite() => Ok(*value),
        Some(FormulaValue::Error(error)) => Err(error.clone()),
        Some(_) => Err(FormulaError::Value),
        None => default.ok_or(FormulaError::Value),
    }
}

fn financial_type(value: f64) -> Result<f64, FormulaError> {
    if (value - 0.0).abs() < f64::EPSILON {
        Ok(0.0)
    } else if (value - 1.0).abs() < f64::EPSILON {
        Ok(1.0)
    } else {
        Err(FormulaError::Value)
    }
}

fn annuity_factor(rate: f64, periods: f64) -> Result<(f64, f64), FormulaError> {
    if !rate.is_finite() || !periods.is_finite() || periods == 0.0 {
        return Err(FormulaError::DivZero);
    }
    if (rate + 1.0).abs() < f64::EPSILON {
        return Err(FormulaError::Value);
    }
    let growth = (1.0 + rate).powf(periods);
    if !growth.is_finite() {
        return Err(FormulaError::Value);
    }
    let factor = if rate.abs() < f64::EPSILON {
        periods
    } else {
        (growth - 1.0) / rate
    };
    if !factor.is_finite() {
        return Err(FormulaError::Value);
    }
    Ok((growth, factor))
}

/// Payment for a fixed-rate annuity. Optional future value and payment timing
/// follow Excel's PMT(rate, nper, pv, [fv], [type]) convention.
fn eval_pmt(args: &[FormulaValue]) -> FormulaValue {
    if !(3..=5).contains(&args.len()) {
        return FormulaValue::Error(FormulaError::Value);
    }
    let rate = match financial_number(args, 0, None) {
        Ok(value) => value,
        Err(error) => return FormulaValue::Error(error),
    };
    let periods = match financial_number(args, 1, None) {
        Ok(value) => value,
        Err(error) => return FormulaValue::Error(error),
    };
    let present = match financial_number(args, 2, None) {
        Ok(value) => value,
        Err(error) => return FormulaValue::Error(error),
    };
    let future = match financial_number(args, 3, Some(0.0)) {
        Ok(value) => value,
        Err(error) => return FormulaValue::Error(error),
    };
    let timing = match financial_number(args, 4, Some(0.0)).and_then(financial_type) {
        Ok(value) => value,
        Err(error) => return FormulaValue::Error(error),
    };
    let (growth, factor) = match annuity_factor(rate, periods) {
        Ok(value) => value,
        Err(error) => return FormulaValue::Error(error),
    };
    let payment = if rate.abs() < f64::EPSILON {
        -(present + future) / periods
    } else {
        -(present * growth + future) / ((1.0 + rate * timing) * factor)
    };
    if payment.is_finite() {
        FormulaValue::Number(payment)
    } else {
        FormulaValue::Error(FormulaError::Value)
    }
}

/// Present value for a fixed-rate annuity. Optional future value and payment
/// timing follow Excel's PV(rate, nper, pmt, [fv], [type]) convention.
fn eval_pv(args: &[FormulaValue]) -> FormulaValue {
    if !(3..=5).contains(&args.len()) {
        return FormulaValue::Error(FormulaError::Value);
    }
    let rate = match financial_number(args, 0, None) {
        Ok(value) => value,
        Err(error) => return FormulaValue::Error(error),
    };
    let periods = match financial_number(args, 1, None) {
        Ok(value) => value,
        Err(error) => return FormulaValue::Error(error),
    };
    let payment = match financial_number(args, 2, None) {
        Ok(value) => value,
        Err(error) => return FormulaValue::Error(error),
    };
    let future = match financial_number(args, 3, Some(0.0)) {
        Ok(value) => value,
        Err(error) => return FormulaValue::Error(error),
    };
    let timing = match financial_number(args, 4, Some(0.0)).and_then(financial_type) {
        Ok(value) => value,
        Err(error) => return FormulaValue::Error(error),
    };
    let (growth, factor) = match annuity_factor(rate, periods) {
        Ok(value) => value,
        Err(error) => return FormulaValue::Error(error),
    };
    let present = if rate.abs() < f64::EPSILON {
        -(future + payment * periods)
    } else {
        -(future + payment * (1.0 + rate * timing) * factor) / growth
    };
    if present.is_finite() {
        FormulaValue::Number(present)
    } else {
        FormulaValue::Error(FormulaError::Value)
    }
}

/// Future value for a fixed-rate annuity. Optional present value and payment
/// timing follow Excel's FV(rate, nper, pmt, [pv], [type]) convention.
fn eval_fv(args: &[FormulaValue]) -> FormulaValue {
    if !(3..=5).contains(&args.len()) {
        return FormulaValue::Error(FormulaError::Value);
    }
    let rate = match financial_number(args, 0, None) {
        Ok(value) => value,
        Err(error) => return FormulaValue::Error(error),
    };
    let periods = match financial_number(args, 1, None) {
        Ok(value) => value,
        Err(error) => return FormulaValue::Error(error),
    };
    let payment = match financial_number(args, 2, None) {
        Ok(value) => value,
        Err(error) => return FormulaValue::Error(error),
    };
    let present = match financial_number(args, 3, Some(0.0)) {
        Ok(value) => value,
        Err(error) => return FormulaValue::Error(error),
    };
    let timing = match financial_number(args, 4, Some(0.0)).and_then(financial_type) {
        Ok(value) => value,
        Err(error) => return FormulaValue::Error(error),
    };
    let (growth, factor) = match annuity_factor(rate, periods) {
        Ok(value) => value,
        Err(error) => return FormulaValue::Error(error),
    };
    let future = if rate.abs() < f64::EPSILON {
        -(present + payment * periods)
    } else {
        -(present * growth + payment * (1.0 + rate * timing) * factor)
    };
    if future.is_finite() {
        FormulaValue::Number(future)
    } else {
        FormulaValue::Error(FormulaError::Value)
    }
}

fn financial_cashflows(value: &FormulaValue, out: &mut Vec<f64>) -> Result<(), FormulaError> {
    match value {
        FormulaValue::Array(values, _, _) => {
            for item in values {
                financial_cashflows(item, out)?;
            }
        }
        FormulaValue::Number(number) if number.is_finite() => out.push(*number),
        FormulaValue::Error(error) => return Err(error.clone()),
        FormulaValue::Empty => {}
        _ => return Err(FormulaError::Value),
    }
    Ok(())
}

/// Net present value discounts each cash flow one period after the previous.
fn eval_npv(args: &[FormulaValue]) -> FormulaValue {
    if args.len() < 2 {
        return FormulaValue::Error(FormulaError::Value);
    }
    let rate = match financial_number(args, 0, None) {
        Ok(value) if value > -1.0 => value,
        Ok(_) => return FormulaValue::Error(FormulaError::Value),
        Err(error) => return FormulaValue::Error(error),
    };
    let mut cashflows = Vec::new();
    for value in &args[1..] {
        if let Err(error) = financial_cashflows(value, &mut cashflows) {
            return FormulaValue::Error(error);
        }
    }
    if cashflows.is_empty() {
        return FormulaValue::Number(0.0);
    }
    let value = cashflows
        .iter()
        .enumerate()
        .map(|(index, cashflow)| cashflow / (1.0 + rate).powi((index + 1) as i32))
        .sum::<f64>();
    if value.is_finite() {
        FormulaValue::Number(value)
    } else {
        FormulaValue::Error(FormulaError::Value)
    }
}

/// Internal-rate-of-return solver using safeguarded Newton iterations with a
/// deterministic positive-rate bracket fallback.
fn eval_irr(args: &[FormulaValue]) -> FormulaValue {
    if !(1..=2).contains(&args.len()) {
        return FormulaValue::Error(FormulaError::Value);
    }
    let mut cashflows = Vec::new();
    if let Err(error) = financial_cashflows(&args[0], &mut cashflows) {
        return FormulaValue::Error(error);
    }
    if cashflows.len() < 2
        || !cashflows.iter().any(|value| *value > 0.0)
        || !cashflows.iter().any(|value| *value < 0.0)
    {
        // Excel reports IRR domain failures as #NUM!.
        return FormulaValue::Error(FormulaError::Num);
    }
    let guess = match financial_number(args, 1, Some(0.1)) {
        Ok(value) if value > -1.0 => value,
        Ok(_) => return FormulaValue::Error(FormulaError::Value),
        Err(error) => return FormulaValue::Error(error),
    };
    let npv = |rate: f64| {
        cashflows
            .iter()
            .enumerate()
            .map(|(index, cashflow)| cashflow / (1.0 + rate).powi(index as i32))
            .sum::<f64>()
    };
    let derivative = |rate: f64| {
        cashflows
            .iter()
            .enumerate()
            .skip(1)
            .map(|(index, cashflow)| {
                -(index as f64) * cashflow / (1.0 + rate).powi((index + 1) as i32)
            })
            .sum::<f64>()
    };

    let mut rate = guess;
    for _ in 0..100 {
        let value = npv(rate);
        if value.abs() < 1e-10 {
            return FormulaValue::Number(rate);
        }
        let slope = derivative(rate);
        if !slope.is_finite() || slope.abs() < 1e-12 {
            break;
        }
        let next = rate - value / slope;
        if !next.is_finite() || next <= -0.999999999 || next > 1.0e6 {
            break;
        }
        rate = next;
    }

    let mut low = -0.999999;
    let mut high = 1.0;
    let mut low_value = npv(low);
    let mut high_value = npv(high);
    for _ in 0..40 {
        if low_value.signum() != high_value.signum() {
            break;
        }
        high *= 2.0;
        high_value = npv(high);
    }
    if low_value.signum() == high_value.signum() {
        return FormulaValue::Error(FormulaError::Value);
    }
    for _ in 0..120 {
        let mid = (low + high) / 2.0;
        let mid_value = npv(mid);
        if mid_value.abs() < 1e-10 {
            return FormulaValue::Number(mid);
        }
        if mid_value.signum() == low_value.signum() {
            low = mid;
            low_value = mid_value;
        } else {
            high = mid;
        }
    }
    let result = (low + high) / 2.0;
    if result.is_finite() {
        FormulaValue::Number(result)
    } else {
        FormulaValue::Error(FormulaError::Value)
    }
}

fn rand_unit() -> f64 {
    let mut hasher = DefaultHasher::new();
    SystemTime::now().hash(&mut hasher);
    thread::current().id().hash(&mut hasher);
    (hasher.finish() % 1_000_000) as f64 / 1_000_000.0
}

fn is_leap_year(year: i32) -> bool {
    year % 400 == 0 || (year % 4 == 0 && year % 100 != 0)
}

fn days_in_month(year: i32, month: i32) -> i32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if is_leap_year(year) {
                29
            } else {
                28
            }
        }
        _ => 30,
    }
}

fn normalize_ymd(year: i32, month: i32, day: i32) -> Option<(i32, i32, i32)> {
    let mut y = year;
    let mut m = month;
    let mut d = day;

    while m > 12 {
        m -= 12;
        y += 1;
    }
    while m < 1 {
        m += 12;
        y -= 1;
    }

    while d > days_in_month(y, m) {
        d -= days_in_month(y, m);
        m += 1;
        if m > 12 {
            m = 1;
            y += 1;
        }
    }
    while d < 1 {
        m -= 1;
        if m < 1 {
            m = 12;
            y -= 1;
        }
        d += days_in_month(y, m);
    }

    Some((y, m, d))
}

fn civil_to_days(year: i32, month: i32, day: i32) -> Option<i64> {
    let (y, m, d) = normalize_ymd(year, month, day)?;

    let mut adj_y = y;
    adj_y -= if m <= 2 { 1 } else { 0 };
    let era = if adj_y >= 0 {
        adj_y / 400
    } else {
        (adj_y - 399) / 400
    };
    let yoe = adj_y - era * 400;
    let doy = (153 * (m + if m > 2 { -3 } else { 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some(era as i64 * 146097 + doe as i64 - 719468)
}

fn days_to_civil(days: i64) -> (i32, i32, i32) {
    let z = days + 719468;
    let era = if z >= 0 {
        z / 146097
    } else {
        (z - 146096) / 146097
    };
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = mp + if mp < 10 { 3 } else { -9 };
    let year = y + if m <= 2 { 1 } else { 0 };
    (year as i32, m as i32, d as i32)
}

fn excel_serial_from_ymd(year: i32, month: i32, day: i32) -> Option<f64> {
    let epoch = civil_to_days(EXCEL_EPOCH_YEAR, EXCEL_EPOCH_MONTH, EXCEL_EPOCH_DAY)?;
    let target = civil_to_days(year, month, day)?;
    Some((target - epoch) as f64)
}

fn excel_ymd_from_serial(serial: f64) -> Option<(i32, i32, i32)> {
    if !serial.is_finite() || serial.floor().abs() > MAX_DATE_SERIAL {
        return None;
    }
    let epoch = civil_to_days(EXCEL_EPOCH_YEAR, EXCEL_EPOCH_MONTH, EXCEL_EPOCH_DAY)?;
    let days = epoch + serial.floor() as i64;
    Some(days_to_civil(days))
}

fn as_number_arg(val: &FormulaValue) -> Option<f64> {
    match val {
        FormulaValue::Number(n) => Some(*n),
        _ => None,
    }
}

fn criterion_string(val: &FormulaValue) -> Option<String> {
    match val {
        FormulaValue::String(s) => Some(s.clone()),
        FormulaValue::Number(n) => Some(n.to_string()),
        FormulaValue::Boolean(b) => Some(b.to_string()),
        FormulaValue::Empty => Some(String::new()),
        FormulaValue::Error(e) => Some(e.to_str().to_string()),
        FormulaValue::Array(..) => None,
    }
}

fn array_dims(args: &FormulaValue) -> Option<(Vec<FormulaValue>, u32, u32)> {
    match args {
        FormulaValue::Array(data, rows, cols) => Some((data.clone(), *rows, *cols)),
        val => Some((vec![val.clone()], 1, 1)),
    }
}

fn eval_rows(args: &[FormulaValue]) -> FormulaValue {
    let Some(value) = args.first() else {
        return FormulaValue::Error(FormulaError::Value);
    };
    match range_values_and_shape(value) {
        Ok((_, rows, _)) => FormulaValue::Number(rows as f64),
        Err(error) => FormulaValue::Error(error),
    }
}

fn eval_columns(args: &[FormulaValue]) -> FormulaValue {
    let Some(value) = args.first() else {
        return FormulaValue::Error(FormulaError::Value);
    };
    match range_values_and_shape(value) {
        Ok((_, _, columns)) => FormulaValue::Number(columns as f64),
        Err(error) => FormulaValue::Error(error),
    }
}

fn eval_transpose(args: &[FormulaValue]) -> FormulaValue {
    const MAX_TRANSPOSE_CELLS: usize = 100_000;
    let Some(value) = args.first() else {
        return FormulaValue::Error(FormulaError::Value);
    };
    let (data, rows, cols) = match range_values_and_shape(value) {
        Ok(shape) => shape,
        Err(error) => return FormulaValue::Error(error),
    };
    let Some(cell_count) = (rows as usize).checked_mul(cols as usize) else {
        return FormulaValue::Error(FormulaError::Value);
    };
    if cell_count > MAX_TRANSPOSE_CELLS {
        return FormulaValue::Error(FormulaError::Value);
    }
    let mut transposed = Vec::with_capacity(cell_count);
    for column in 0..cols as usize {
        for row in 0..rows as usize {
            let Some(value) = data.get(row * cols as usize + column) else {
                return FormulaValue::Error(FormulaError::Value);
            };
            transposed.push(value.clone());
        }
    }
    FormulaValue::Array(transposed, cols, rows)
}

fn eval_to_row_or_column(args: &[FormulaValue], as_column: bool) -> FormulaValue {
    if args.is_empty() || args.len() > 3 {
        return FormulaValue::Error(FormulaError::Value);
    }
    let (data, rows, cols) = match range_values_and_shape(&args[0]) {
        Ok(shape) => shape,
        Err(error) => return FormulaValue::Error(error),
    };
    if data.len() > MAX_DYNAMIC_ARRAY_CELLS {
        return FormulaValue::Error(FormulaError::Value);
    }
    let ignore = match args.get(1) {
        None | Some(FormulaValue::Empty) => 0,
        Some(FormulaValue::Number(value))
            if value.is_finite() && value.fract() == 0.0 && (0.0..=3.0).contains(value) =>
        {
            *value as u8
        }
        Some(FormulaValue::Error(error)) => return FormulaValue::Error(error.clone()),
        _ => return FormulaValue::Error(FormulaError::Value),
    };
    let scan_by_column = match optional_boolean(args.get(2), false) {
        Ok(value) => value,
        Err(error) => return FormulaValue::Error(error),
    };

    let mut output = Vec::with_capacity(data.len());
    let mut push_value = |value: &FormulaValue| {
        let blank = matches!(value, FormulaValue::Empty)
            || matches!(value, FormulaValue::String(text) if text.is_empty());
        let ignored = match ignore {
            1 => blank,
            2 => matches!(value, FormulaValue::Error(_)),
            3 => blank || matches!(value, FormulaValue::Error(_)),
            _ => false,
        };
        if !ignored {
            output.push(value.clone());
        }
    };
    if scan_by_column {
        for column in 0..cols as usize {
            for row in 0..rows as usize {
                let Some(value) = data.get(row * cols as usize + column) else {
                    return FormulaValue::Error(FormulaError::Value);
                };
                push_value(value);
            }
        }
    } else {
        for value in &data {
            push_value(value);
        }
    }
    if output.is_empty() {
        return FormulaValue::Error(FormulaError::Na);
    }
    let Some(output_len) = u32::try_from(output.len()).ok() else {
        return FormulaValue::Error(FormulaError::Value);
    };
    if as_column {
        FormulaValue::Array(output, output_len, 1)
    } else {
        FormulaValue::Array(output, 1, output_len)
    }
}

fn eval_wrap(args: &[FormulaValue], by_rows: bool) -> FormulaValue {
    if args.len() < 2 || args.len() > 3 {
        return FormulaValue::Error(FormulaError::Value);
    }
    let (data, _, _) = match range_values_and_shape(&args[0]) {
        Ok(shape) => shape,
        Err(error) => return FormulaValue::Error(error),
    };
    if data.is_empty() || data.len() > MAX_DYNAMIC_ARRAY_CELLS {
        return FormulaValue::Error(FormulaError::Value);
    }
    let wrap_count = match sequence_dimension(&args[1]) {
        Ok(value) if value > 0 => value,
        Err(error) => return FormulaValue::Error(error),
        Ok(_) => return FormulaValue::Error(FormulaError::Value),
    };
    let pad = match args.get(2) {
        None => FormulaValue::Error(FormulaError::Na),
        Some(FormulaValue::Array(..)) => return FormulaValue::Error(FormulaError::Value),
        Some(value) => value.clone(),
    };
    let output_outer = data.len().saturating_add(wrap_count.saturating_sub(1)) / wrap_count;
    let (rows, cols) = if by_rows {
        (output_outer, wrap_count)
    } else {
        (wrap_count, output_outer)
    };
    let Some(cell_count) = rows.checked_mul(cols) else {
        return FormulaValue::Error(FormulaError::Value);
    };
    if cell_count > MAX_DYNAMIC_ARRAY_CELLS {
        return FormulaValue::Error(FormulaError::Value);
    }
    let Some(rows) = u32::try_from(rows).ok() else {
        return FormulaValue::Error(FormulaError::Value);
    };
    let Some(cols) = u32::try_from(cols).ok() else {
        return FormulaValue::Error(FormulaError::Value);
    };
    let mut output = Vec::with_capacity(cell_count);
    if by_rows {
        for row in 0..rows as usize {
            for column in 0..cols as usize {
                let index = row * cols as usize + column;
                output.push(data.get(index).cloned().unwrap_or_else(|| pad.clone()));
            }
        }
    } else {
        for row in 0..rows as usize {
            for column in 0..cols as usize {
                let index = column * rows as usize + row;
                output.push(data.get(index).cloned().unwrap_or_else(|| pad.clone()));
            }
        }
    }
    FormulaValue::Array(output, rows, cols)
}

pub const MAX_DYNAMIC_ARRAY_CELLS: usize = 100_000;

fn sequence_dimension(value: &FormulaValue) -> Result<usize, FormulaError> {
    let FormulaValue::Number(value) = value else {
        return match value {
            FormulaValue::Error(error) => Err(error.clone()),
            _ => Err(FormulaError::Value),
        };
    };
    if !value.is_finite() || *value < 1.0 || value.fract() != 0.0 {
        return Err(FormulaError::Value);
    }
    usize::try_from(*value as u128).map_err(|_| FormulaError::Value)
}

fn sequence_number(value: Option<&FormulaValue>, default: f64) -> Result<f64, FormulaError> {
    match value {
        None => Ok(default),
        Some(FormulaValue::Empty) => Ok(default),
        Some(FormulaValue::Number(value)) if value.is_finite() => Ok(*value),
        Some(FormulaValue::Error(error)) => Err(error.clone()),
        _ => Err(FormulaError::Value),
    }
}

fn eval_sequence(args: &[FormulaValue]) -> FormulaValue {
    if args.is_empty() || args.len() > 4 {
        return FormulaValue::Error(FormulaError::Value);
    }
    let rows = match sequence_dimension(&args[0]) {
        Ok(value) if value > 0 => value,
        Err(error) => return FormulaValue::Error(error),
        Ok(_) => return FormulaValue::Error(FormulaError::Value),
    };
    let columns = match args.get(1) {
        None => 1,
        Some(FormulaValue::Empty) => 1,
        Some(value) => match sequence_dimension(value) {
            Ok(value) if value > 0 => value,
            Err(error) => return FormulaValue::Error(error),
            Ok(_) => return FormulaValue::Error(FormulaError::Value),
        },
    };
    let Some(cell_count) = rows.checked_mul(columns) else {
        return FormulaValue::Error(FormulaError::Value);
    };
    if cell_count > MAX_DYNAMIC_ARRAY_CELLS {
        return FormulaValue::Error(FormulaError::Value);
    }
    let start = match sequence_number(args.get(2), 1.0) {
        Ok(value) => value,
        Err(error) => return FormulaValue::Error(error),
    };
    let step = match sequence_number(args.get(3), 1.0) {
        Ok(value) => value,
        Err(error) => return FormulaValue::Error(error),
    };
    let mut output = Vec::with_capacity(cell_count);
    for index in 0..cell_count {
        let value = start + (index as f64) * step;
        if !value.is_finite() {
            return FormulaValue::Error(FormulaError::Value);
        }
        output.push(FormulaValue::Number(value));
    }
    FormulaValue::Array(output, rows as u32, columns as u32)
}

fn optional_array_dimension(value: Option<&FormulaValue>) -> Result<usize, FormulaError> {
    match value {
        None | Some(FormulaValue::Empty) => Ok(1),
        Some(value) => sequence_dimension(value),
    }
}

fn optional_boolean(value: Option<&FormulaValue>, default: bool) -> Result<bool, FormulaError> {
    match value {
        None | Some(FormulaValue::Empty) => Ok(default),
        Some(FormulaValue::Boolean(value)) => Ok(*value),
        Some(FormulaValue::Number(value)) if value.is_finite() => Ok(*value != 0.0),
        Some(FormulaValue::Error(error)) => Err(error.clone()),
        _ => Err(FormulaError::Value),
    }
}

fn eval_randarray(args: &[FormulaValue]) -> FormulaValue {
    if args.len() > 5 {
        return FormulaValue::Error(FormulaError::Value);
    }
    let rows = match optional_array_dimension(args.first()) {
        Ok(value) if value > 0 => value,
        Ok(_) => return FormulaValue::Error(FormulaError::Value),
        Err(error) => return FormulaValue::Error(error),
    };
    let columns = match optional_array_dimension(args.get(1)) {
        Ok(value) if value > 0 => value,
        Ok(_) => return FormulaValue::Error(FormulaError::Value),
        Err(error) => return FormulaValue::Error(error),
    };
    let Some(cell_count) = rows.checked_mul(columns) else {
        return FormulaValue::Error(FormulaError::Value);
    };
    if cell_count > MAX_DYNAMIC_ARRAY_CELLS {
        return FormulaValue::Error(FormulaError::Value);
    }
    let min = match sequence_number(args.get(2), 0.0) {
        Ok(value) => value,
        Err(error) => return FormulaValue::Error(error),
    };
    let max = match sequence_number(args.get(3), 1.0) {
        Ok(value) => value,
        Err(error) => return FormulaValue::Error(error),
    };
    if min > max {
        return FormulaValue::Error(FormulaError::Value);
    }
    let whole_number = match optional_boolean(args.get(4), false) {
        Ok(value) => value,
        Err(error) => return FormulaValue::Error(error),
    };
    let mut output = Vec::with_capacity(cell_count);
    if whole_number {
        let low = min.ceil();
        let high = max.floor();
        if !low.is_finite() || !high.is_finite() || low > high {
            return FormulaValue::Error(FormulaError::Value);
        }
        let span = high - low + 1.0;
        if !span.is_finite() || span <= 0.0 {
            return FormulaValue::Error(FormulaError::Value);
        }
        for _ in 0..cell_count {
            let value = low + (rand_unit() * span).floor();
            if !value.is_finite() {
                return FormulaValue::Error(FormulaError::Value);
            }
            output.push(FormulaValue::Number(value));
        }
    } else {
        let span = max - min;
        if !span.is_finite() {
            return FormulaValue::Error(FormulaError::Value);
        }
        for _ in 0..cell_count {
            let value = min + rand_unit() * span;
            if !value.is_finite() {
                return FormulaValue::Error(FormulaError::Value);
            }
            output.push(FormulaValue::Number(value));
        }
    }
    FormulaValue::Array(output, rows as u32, columns as u32)
}

fn stack_arrays(args: &[FormulaValue], horizontal: bool) -> FormulaValue {
    if args.is_empty() || args.len() > 255 {
        return FormulaValue::Error(FormulaError::Value);
    }
    let mut arrays = Vec::with_capacity(args.len());
    for value in args {
        let (data, rows, cols) = match range_values_and_shape(value) {
            Ok(shape) => shape,
            Err(error) => return FormulaValue::Error(error),
        };
        if rows == 0 || cols == 0 {
            return FormulaValue::Error(FormulaError::Value);
        }
        arrays.push((data, rows as usize, cols as usize));
    }
    let (rows, cols) = if horizontal {
        let rows = arrays.iter().map(|(_, rows, _)| *rows).max().unwrap_or(0);
        let cols = arrays
            .iter()
            .try_fold(0usize, |total, (_, _, cols)| total.checked_add(*cols));
        (Some(rows), cols)
    } else {
        let rows = arrays
            .iter()
            .try_fold(0usize, |total, (_, rows, _)| total.checked_add(*rows));
        let cols = arrays.iter().map(|(_, _, cols)| *cols).max().unwrap_or(0);
        (rows, Some(cols))
    };
    let Some(rows) = rows else {
        return FormulaValue::Error(FormulaError::Value);
    };
    let Some(cols) = cols else {
        return FormulaValue::Error(FormulaError::Value);
    };
    let Some(cell_count) = rows.checked_mul(cols) else {
        return FormulaValue::Error(FormulaError::Value);
    };
    if rows > u32::MAX as usize || cols > u32::MAX as usize || cell_count > MAX_DYNAMIC_ARRAY_CELLS
    {
        return FormulaValue::Error(FormulaError::Value);
    }

    let mut output = vec![FormulaValue::Empty; cell_count];
    if horizontal {
        let mut col_offset = 0usize;
        for (data, source_rows, source_cols) in arrays {
            for row in 0..source_rows {
                for col in 0..source_cols {
                    let source_index = row * source_cols + col;
                    let target_index = row * cols + col_offset + col;
                    let Some(value) = data.get(source_index) else {
                        return FormulaValue::Error(FormulaError::Value);
                    };
                    output[target_index] = value.clone();
                }
            }
            col_offset += source_cols;
        }
    } else {
        let mut row_offset = 0usize;
        for (data, source_rows, source_cols) in arrays {
            for row in 0..source_rows {
                for col in 0..source_cols {
                    let source_index = row * source_cols + col;
                    let target_index = (row_offset + row) * cols + col;
                    let Some(value) = data.get(source_index) else {
                        return FormulaValue::Error(FormulaError::Value);
                    };
                    output[target_index] = value.clone();
                }
            }
            row_offset += source_rows;
        }
    }
    FormulaValue::Array(output, rows as u32, cols as u32)
}

fn eval_hstack(args: &[FormulaValue]) -> FormulaValue {
    stack_arrays(args, true)
}

fn eval_vstack(args: &[FormulaValue]) -> FormulaValue {
    stack_arrays(args, false)
}

fn integer_array_argument(value: &FormulaValue) -> Result<i64, FormulaError> {
    match value {
        FormulaValue::Number(value)
            if value.is_finite()
                && value.fract() == 0.0
                && *value >= i64::MIN as f64
                && *value <= i64::MAX as f64 =>
        {
            Ok(*value as i64)
        }
        FormulaValue::Error(error) => Err(error.clone()),
        _ => Err(FormulaError::Value),
    }
}

fn eval_take_drop(args: &[FormulaValue], take: bool) -> FormulaValue {
    if !(2..=3).contains(&args.len()) {
        return FormulaValue::Error(FormulaError::Value);
    }
    let (data, rows, cols) = match range_values_and_shape(&args[0]) {
        Ok(shape) => shape,
        Err(error) => return FormulaValue::Error(error),
    };
    let row_count = match integer_array_argument(&args[1]) {
        Ok(value) if value != 0 => value,
        Ok(_) => return FormulaValue::Error(FormulaError::Value),
        Err(error) => return FormulaValue::Error(error),
    };
    let col_count = match args.get(2) {
        None | Some(FormulaValue::Empty) => {
            if take {
                i64::from(cols)
            } else {
                0
            }
        }
        Some(value) => match integer_array_argument(value) {
            Ok(value) if value != 0 => value,
            Ok(_) => return FormulaValue::Error(FormulaError::Value),
            Err(error) => return FormulaValue::Error(error),
        },
    };
    let total_rows = i64::from(rows);
    let total_cols = i64::from(cols);
    let (row_start, row_end) = if take {
        if row_count > 0 {
            (0, row_count.min(total_rows))
        } else {
            ((total_rows + row_count).max(0), total_rows)
        }
    } else if row_count > 0 {
        (row_count.min(total_rows), total_rows)
    } else {
        (0, (total_rows + row_count).max(0))
    };
    let (col_start, col_end) = if col_count == 0 {
        (0, total_cols)
    } else if take {
        if col_count > 0 {
            (0, col_count.min(total_cols))
        } else {
            ((total_cols + col_count).max(0), total_cols)
        }
    } else if col_count > 0 {
        (col_count.min(total_cols), total_cols)
    } else {
        (0, (total_cols + col_count).max(0))
    };
    if row_start >= row_end || col_start >= col_end {
        return FormulaValue::Error(FormulaError::Value);
    }
    let output_rows = usize::try_from(row_end - row_start).unwrap_or(0);
    let output_cols = usize::try_from(col_end - col_start).unwrap_or(0);
    let Some(cell_count) = output_rows.checked_mul(output_cols) else {
        return FormulaValue::Error(FormulaError::Value);
    };
    if cell_count > MAX_DYNAMIC_ARRAY_CELLS {
        return FormulaValue::Error(FormulaError::Value);
    }
    let mut output = Vec::with_capacity(cell_count);
    for row in row_start as usize..row_end as usize {
        for col in col_start as usize..col_end as usize {
            let Some(value) = data.get(row * cols as usize + col) else {
                return FormulaValue::Error(FormulaError::Value);
            };
            output.push(value.clone());
        }
    }
    FormulaValue::Array(output, output_rows as u32, output_cols as u32)
}

fn eval_choose_axis(args: &[FormulaValue], rows_axis: bool) -> FormulaValue {
    if args.len() < 2 || args.len() > 255 {
        return FormulaValue::Error(FormulaError::Value);
    }
    let (data, rows, cols) = match range_values_and_shape(&args[0]) {
        Ok(shape) => shape,
        Err(error) => return FormulaValue::Error(error),
    };
    let axis_len = if rows_axis {
        i64::from(rows)
    } else {
        i64::from(cols)
    };
    let mut selectors = Vec::with_capacity(args.len() - 1);
    for value in &args[1..] {
        let selector = match integer_array_argument(value) {
            Ok(value) if value != 0 => value,
            Ok(_) => return FormulaValue::Error(FormulaError::Value),
            Err(error) => return FormulaValue::Error(error),
        };
        let index = if selector > 0 {
            selector - 1
        } else {
            axis_len + selector
        };
        if index < 0 || index >= axis_len {
            return FormulaValue::Error(FormulaError::Value);
        }
        selectors.push(index as usize);
    }
    let output_rows = if rows_axis {
        selectors.len()
    } else {
        rows as usize
    };
    let output_cols = if rows_axis {
        cols as usize
    } else {
        selectors.len()
    };
    let Some(cell_count) = output_rows.checked_mul(output_cols) else {
        return FormulaValue::Error(FormulaError::Value);
    };
    if cell_count > MAX_DYNAMIC_ARRAY_CELLS {
        return FormulaValue::Error(FormulaError::Value);
    }
    let mut output = Vec::with_capacity(cell_count);
    if rows_axis {
        for &source_row in &selectors {
            for col in 0..cols as usize {
                let Some(value) = data.get(source_row * cols as usize + col) else {
                    return FormulaValue::Error(FormulaError::Value);
                };
                output.push(value.clone());
            }
        }
    } else {
        for row in 0..rows as usize {
            for &source_col in &selectors {
                let Some(value) = data.get(row * cols as usize + source_col) else {
                    return FormulaValue::Error(FormulaError::Value);
                };
                output.push(value.clone());
            }
        }
    }
    FormulaValue::Array(output, output_rows as u32, output_cols as u32)
}

fn array_truthy(value: &FormulaValue) -> Result<bool, FormulaError> {
    match value {
        FormulaValue::Boolean(value) => Ok(*value),
        FormulaValue::Number(value) if value.is_finite() => Ok(*value != 0.0),
        FormulaValue::Number(_) => Err(FormulaError::Value),
        FormulaValue::Empty => Ok(false),
        FormulaValue::String(value) => Ok(!value.is_empty()),
        FormulaValue::Error(error) => Err(error.clone()),
        FormulaValue::Array(..) => Err(FormulaError::Value),
    }
}

fn eval_filter(args: &[FormulaValue]) -> FormulaValue {
    if args.len() < 2 {
        return FormulaValue::Error(FormulaError::Value);
    }
    let (data, rows, cols) = match range_values_and_shape(&args[0]) {
        Ok(shape) => shape,
        Err(error) => return FormulaValue::Error(error),
    };
    if data.len() > MAX_DYNAMIC_ARRAY_CELLS {
        return FormulaValue::Error(FormulaError::Value);
    }
    let (include, include_rows, include_cols) = match range_values_and_shape(&args[1]) {
        Ok(shape) => shape,
        Err(error) => return FormulaValue::Error(error),
    };
    let row_filter = include_cols == 1 && include_rows == rows;
    let column_filter = include_rows == 1 && include_cols == cols;
    if !row_filter && !column_filter {
        return FormulaValue::Error(FormulaError::Value);
    }

    let mut selected = Vec::new();
    let mut selected_count = 0usize;
    if row_filter {
        for row in 0..rows as usize {
            let keep = match array_truthy(include.get(row).unwrap_or(&FormulaValue::Empty)) {
                Ok(value) => value,
                Err(error) => return FormulaValue::Error(error),
            };
            if keep {
                selected_count += 1;
                let start = row * cols as usize;
                let end = start.saturating_add(cols as usize);
                let Some(values) = data.get(start..end) else {
                    return FormulaValue::Error(FormulaError::Value);
                };
                selected.extend(values.iter().cloned());
            }
        }
        if selected_count == 0 {
            return args
                .get(2)
                .cloned()
                .unwrap_or(FormulaValue::Error(FormulaError::Na));
        }
        if selected.len() > MAX_DYNAMIC_ARRAY_CELLS {
            return FormulaValue::Error(FormulaError::Value);
        }
        return FormulaValue::Array(selected, selected_count as u32, cols);
    }

    for column in 0..cols as usize {
        let keep = match array_truthy(include.get(column).unwrap_or(&FormulaValue::Empty)) {
            Ok(value) => value,
            Err(error) => return FormulaValue::Error(error),
        };
        if keep {
            selected_count += 1;
            for row in 0..rows as usize {
                let Some(value) = data.get(row * cols as usize + column) else {
                    return FormulaValue::Error(FormulaError::Value);
                };
                selected.push(value.clone());
            }
        }
    }
    if selected_count == 0 {
        return args
            .get(2)
            .cloned()
            .unwrap_or(FormulaValue::Error(FormulaError::Na));
    }
    if selected.len() > MAX_DYNAMIC_ARRAY_CELLS {
        return FormulaValue::Error(FormulaError::Value);
    }
    FormulaValue::Array(selected, rows, selected_count as u32)
}

fn value_equal_for_array(left: &FormulaValue, right: &FormulaValue) -> bool {
    if matches!((left, right), (FormulaValue::Empty, FormulaValue::Empty)) {
        return true;
    }
    if let (FormulaValue::Error(left), FormulaValue::Error(right)) = (left, right) {
        return left == right;
    }
    values_equal(left, right)
}

fn array_bool_arg(value: Option<&FormulaValue>, default: bool) -> Result<bool, FormulaError> {
    match value {
        None => Ok(default),
        Some(FormulaValue::Boolean(value)) => Ok(*value),
        Some(FormulaValue::Number(value)) if value.is_finite() => Ok(*value != 0.0),
        Some(FormulaValue::Error(error)) => Err(error.clone()),
        _ => Err(FormulaError::Value),
    }
}

fn eval_unique(args: &[FormulaValue]) -> FormulaValue {
    let Some(value) = args.first() else {
        return FormulaValue::Error(FormulaError::Value);
    };
    let (data, rows, cols) = match range_values_and_shape(value) {
        Ok(shape) => shape,
        Err(error) => return FormulaValue::Error(error),
    };
    if data.len() > MAX_DYNAMIC_ARRAY_CELLS {
        return FormulaValue::Error(FormulaError::Value);
    }
    let by_column = match array_bool_arg(args.get(1), false) {
        Ok(value) => value,
        Err(error) => return FormulaValue::Error(error),
    };
    let exactly_once = match array_bool_arg(args.get(2), false) {
        Ok(value) => value,
        Err(error) => return FormulaValue::Error(error),
    };
    let outer = if by_column { cols } else { rows } as usize;
    let inner = if by_column { rows } else { cols } as usize;
    let mut groups: Vec<Vec<FormulaValue>> = Vec::new();
    let mut counts: Vec<usize> = Vec::new();
    for outer_index in 0..outer {
        let mut group = Vec::with_capacity(inner);
        for inner_index in 0..inner {
            let index = if by_column {
                inner_index * cols as usize + outer_index
            } else {
                outer_index * cols as usize + inner_index
            };
            let Some(value) = data.get(index) else {
                return FormulaValue::Error(FormulaError::Value);
            };
            group.push(value.clone());
        }
        if let Some(existing) = groups.iter().position(|candidate| {
            candidate
                .iter()
                .zip(&group)
                .all(|(left, right)| value_equal_for_array(left, right))
        }) {
            counts[existing] += 1;
        } else {
            groups.push(group);
            counts.push(1);
        }
    }
    if exactly_once {
        let filtered = groups
            .into_iter()
            .zip(counts)
            .filter_map(|(group, count)| (count == 1).then_some(group))
            .collect::<Vec<_>>();
        groups = filtered;
    }
    if groups.is_empty() {
        return FormulaValue::Error(FormulaError::Na);
    }
    let output_cells = groups.len().saturating_mul(inner);
    if output_cells > MAX_DYNAMIC_ARRAY_CELLS {
        return FormulaValue::Error(FormulaError::Value);
    }
    let mut output = Vec::with_capacity(output_cells);
    let group_count = groups.len() as u32;
    if by_column {
        for inner_index in 0..inner {
            for group in &groups {
                output.push(group[inner_index].clone());
            }
        }
        FormulaValue::Array(output, rows, group_count)
    } else {
        for group in groups {
            output.extend(group);
        }
        FormulaValue::Array(output, group_count, cols)
    }
}

fn compare_array_values(left: &FormulaValue, right: &FormulaValue) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    match (left, right) {
        (FormulaValue::Number(left), FormulaValue::Number(right)) => {
            left.partial_cmp(right).unwrap_or(Ordering::Equal)
        }
        (FormulaValue::Boolean(left), FormulaValue::Boolean(right)) => left.cmp(right),
        (FormulaValue::String(left), FormulaValue::String(right)) => {
            left.to_ascii_lowercase().cmp(&right.to_ascii_lowercase())
        }
        (FormulaValue::Empty, FormulaValue::Empty) => Ordering::Equal,
        (FormulaValue::Empty, _) => Ordering::Less,
        (_, FormulaValue::Empty) => Ordering::Greater,
        (FormulaValue::Error(left), FormulaValue::Error(right)) => {
            left.to_str().cmp(right.to_str())
        }
        (FormulaValue::Error(_), _) => Ordering::Greater,
        (_, FormulaValue::Error(_)) => Ordering::Less,
        (FormulaValue::Number(left), FormulaValue::String(right)) => right
            .parse::<f64>()
            .ok()
            .and_then(|value| left.partial_cmp(&value))
            .unwrap_or_else(|| left.to_string().cmp(right)),
        (FormulaValue::String(left), FormulaValue::Number(right)) => right
            .partial_cmp(&left.parse::<f64>().unwrap_or(f64::INFINITY))
            .map(Ordering::reverse)
            .unwrap_or_else(|| left.cmp(&right.to_string())),
        (FormulaValue::Boolean(left), FormulaValue::Number(right)) => (*left as u8 as f64)
            .partial_cmp(right)
            .unwrap_or(Ordering::Equal),
        (FormulaValue::Number(left), FormulaValue::Boolean(right)) => left
            .partial_cmp(&(*right as u8 as f64))
            .unwrap_or(Ordering::Equal),
        (FormulaValue::Boolean(left), FormulaValue::String(right)) => {
            left.to_string().cmp(&right.to_ascii_lowercase())
        }
        (FormulaValue::String(left), FormulaValue::Boolean(right)) => {
            left.to_ascii_lowercase().cmp(&right.to_string())
        }
        (FormulaValue::Array(..), FormulaValue::Array(..)) => Ordering::Equal,
        (FormulaValue::Array(..), _) => Ordering::Greater,
        (_, FormulaValue::Array(..)) => Ordering::Less,
    }
}

fn eval_sort(args: &[FormulaValue]) -> FormulaValue {
    let Some(value) = args.first() else {
        return FormulaValue::Error(FormulaError::Value);
    };
    let (data, rows, cols) = match range_values_and_shape(value) {
        Ok(shape) => shape,
        Err(error) => return FormulaValue::Error(error),
    };
    if data.len() > MAX_DYNAMIC_ARRAY_CELLS {
        return FormulaValue::Error(FormulaError::Value);
    }
    let sort_index = match args.get(1) {
        None => 1,
        Some(value) => match as_number_arg(value) {
            Some(value) if value.is_finite() => value.floor() as i64,
            _ => return FormulaValue::Error(FormulaError::Value),
        },
    };
    let descending = match args.get(2) {
        None => false,
        Some(value) => match as_number_arg(value) {
            Some(1.0) => false,
            Some(-1.0) => true,
            _ => return FormulaValue::Error(FormulaError::Value),
        },
    };
    let by_column = match array_bool_arg(args.get(3), false) {
        Ok(value) => value,
        Err(error) => return FormulaValue::Error(error),
    };
    let outer = if by_column { cols } else { rows } as usize;
    let inner = if by_column { rows } else { cols } as usize;
    if sort_index < 1 || sort_index as usize > inner {
        return FormulaValue::Error(FormulaError::Value);
    }
    let key_index = sort_index as usize - 1;
    let mut order: Vec<usize> = (0..outer).collect();
    order.sort_by(|left, right| {
        let left_index = if by_column {
            key_index * cols as usize + *left
        } else {
            *left * cols as usize + key_index
        };
        let right_index = if by_column {
            key_index * cols as usize + *right
        } else {
            *right * cols as usize + key_index
        };
        let left_value = data.get(left_index).unwrap_or(&FormulaValue::Empty);
        let right_value = data.get(right_index).unwrap_or(&FormulaValue::Empty);
        let ordering = compare_array_values(left_value, right_value);
        if descending {
            ordering.reverse()
        } else {
            ordering
        }
    });
    let output_cells = outer.saturating_mul(inner);
    if output_cells > MAX_DYNAMIC_ARRAY_CELLS {
        return FormulaValue::Error(FormulaError::Value);
    }
    let mut output = Vec::with_capacity(output_cells);
    if by_column {
        for row in 0..rows as usize {
            for column in &order {
                let index = row * cols as usize + *column;
                let Some(value) = data.get(index) else {
                    return FormulaValue::Error(FormulaError::Value);
                };
                output.push(value.clone());
            }
        }
    } else {
        for row in order {
            for column in 0..cols as usize {
                let index = row * cols as usize + column;
                let Some(value) = data.get(index) else {
                    return FormulaValue::Error(FormulaError::Value);
                };
                output.push(value.clone());
            }
        }
    }
    FormulaValue::Array(output, rows, cols)
}

fn eval_sortby(args: &[FormulaValue]) -> FormulaValue {
    if args.len() < 2 || args.len() > 255 {
        return FormulaValue::Error(FormulaError::Value);
    }
    let (data, rows, cols) = match range_values_and_shape(&args[0]) {
        Ok(shape) => shape,
        Err(error) => return FormulaValue::Error(error),
    };
    if data.is_empty() || data.len() > MAX_DYNAMIC_ARRAY_CELLS {
        return FormulaValue::Error(FormulaError::Value);
    }

    // SORTBY accepts one-dimensional sort arrays. Keep all keys on the same
    // axis so the result remains rectangular and bounded.
    let mut sort_rows: Option<bool> = None;
    let mut keys: Vec<(Vec<FormulaValue>, bool)> = Vec::new();
    let mut cursor = 1usize;
    while cursor < args.len() {
        let (key_values, key_rows, key_cols) = match range_values_and_shape(&args[cursor]) {
            Ok(shape) => shape,
            Err(error) => return FormulaValue::Error(error),
        };
        let key_is_rows = key_rows == rows && key_cols == 1;
        let key_is_columns = key_rows == 1 && key_cols == cols;
        if !key_is_rows && !key_is_columns {
            return FormulaValue::Error(FormulaError::Value);
        }
        if let Some(expected_rows) = sort_rows {
            if expected_rows != key_is_rows {
                return FormulaValue::Error(FormulaError::Value);
            }
        } else {
            sort_rows = Some(key_is_rows);
        }

        let mut descending = false;
        if let Some(order) = args.get(cursor + 1) {
            match order {
                FormulaValue::Number(value) if value.is_finite() && *value == 1.0 => {
                    cursor += 1;
                }
                FormulaValue::Number(value) if value.is_finite() && *value == -1.0 => {
                    descending = true;
                    cursor += 1;
                }
                FormulaValue::Number(_) => return FormulaValue::Error(FormulaError::Value),
                FormulaValue::Empty => {
                    cursor += 1;
                }
                FormulaValue::Error(error) => return FormulaValue::Error(error.clone()),
                // An array is the next sort key, not a sort-order argument.
                FormulaValue::Array(..) => {}
                _ => return FormulaValue::Error(FormulaError::Value),
            }
        }
        keys.push((key_values, descending));
        cursor += 1;
    }

    let sort_rows = sort_rows.unwrap_or(true);
    let outer = if sort_rows { rows } else { cols } as usize;
    let inner = if sort_rows { cols } else { rows } as usize;
    let mut order: Vec<usize> = (0..outer).collect();
    order.sort_by(|left, right| {
        for (key_values, descending) in &keys {
            let left_value = key_values.get(*left).unwrap_or(&FormulaValue::Empty);
            let right_value = key_values.get(*right).unwrap_or(&FormulaValue::Empty);
            let comparison = compare_array_values(left_value, right_value);
            if comparison != std::cmp::Ordering::Equal {
                return if *descending {
                    comparison.reverse()
                } else {
                    comparison
                };
            }
        }
        std::cmp::Ordering::Equal
    });

    let output_cells = outer.saturating_mul(inner);
    if output_cells > MAX_DYNAMIC_ARRAY_CELLS {
        return FormulaValue::Error(FormulaError::Value);
    }
    let mut output = Vec::with_capacity(output_cells);
    if sort_rows {
        for row in order {
            for column in 0..cols as usize {
                let index = row * cols as usize + column;
                let Some(value) = data.get(index) else {
                    return FormulaValue::Error(FormulaError::Value);
                };
                output.push(value.clone());
            }
        }
    } else {
        for row in 0..rows as usize {
            for column in &order {
                let index = row * cols as usize + *column;
                let Some(value) = data.get(index) else {
                    return FormulaValue::Error(FormulaError::Value);
                };
                output.push(value.clone());
            }
        }
    }
    FormulaValue::Array(output, rows, cols)
}

fn eval_xlookup(args: &[FormulaValue]) -> FormulaValue {
    if args.len() < 3 {
        return FormulaValue::Error(FormulaError::Value);
    }
    let lookup = match &args[0] {
        FormulaValue::Array(data, _, _) => data.first().cloned().unwrap_or(FormulaValue::Empty),
        v => v.clone(),
    };
    let match_mode = match args.get(4) {
        None => 0,
        Some(FormulaValue::Number(n)) => *n as i32,
        Some(FormulaValue::Error(e)) => return FormulaValue::Error(e.clone()),
        _ => 0,
    };
    if match_mode != 0 {
        return FormulaValue::Error(FormulaError::Value);
    }

    let (lookup_data, lookup_rows, lookup_cols) = array_dims(&args[1]).unwrap_or((vec![], 0, 0));
    let (return_data, return_rows, return_cols) = array_dims(&args[2]).unwrap_or((vec![], 0, 0));

    if lookup_data.is_empty() || return_data.is_empty() {
        return FormulaValue::Error(FormulaError::Na);
    }

    let vertical = lookup_cols == 1 && lookup_rows > 1;
    let horizontal = lookup_rows == 1 && lookup_cols > 1;

    let len = if vertical {
        lookup_rows as usize
    } else if horizontal {
        lookup_cols as usize
    } else {
        lookup_data.len()
    };

    for i in 0..len {
        let lookup_idx = i;
        let lookup_val = lookup_data
            .get(lookup_idx)
            .cloned()
            .unwrap_or(FormulaValue::Empty);
        if values_equal(&lookup_val, &lookup) {
            let return_idx =
                if (return_rows == 1 && return_cols > 1) || (return_cols == 1 && return_rows > 1) {
                    i
                } else {
                    i * return_cols as usize
                };
            if let Some(result) = return_data.get(return_idx) {
                return result.clone();
            }
            return FormulaValue::Error(FormulaError::Na);
        }
    }

    if let Some(not_found) = args.get(3) {
        return not_found.clone();
    }
    FormulaValue::Error(FormulaError::Na)
}

fn eval_index(args: &[FormulaValue]) -> FormulaValue {
    if args.len() < 2 {
        return FormulaValue::Error(FormulaError::Value);
    }
    let (data, rows, cols) = array_dims(&args[0]).unwrap_or((vec![], 0, 0));
    let row_num = match as_number_arg(&args[1]) {
        Some(n) => n as i32,
        None => return FormulaValue::Error(FormulaError::Value),
    };
    let col_num = match args.get(2) {
        Some(v) => match as_number_arg(v) {
            Some(n) => n as i32,
            None => return FormulaValue::Error(FormulaError::Value),
        },
        None => 1,
    };
    if row_num < 1 || col_num < 1 {
        return FormulaValue::Error(FormulaError::Value);
    }
    if rows == 1 && cols > 1 {
        if args.len() >= 3 && row_num != 1 {
            return FormulaValue::Error(FormulaError::Ref);
        }
        let idx = ((if args.len() < 3 { row_num } else { col_num }) - 1) as usize;
        return data
            .get(idx)
            .cloned()
            .unwrap_or(FormulaValue::Error(FormulaError::Ref));
    }
    if cols == 1 && rows > 1 {
        if args.len() >= 3 && col_num != 1 {
            return FormulaValue::Error(FormulaError::Ref);
        }
        let idx = (row_num - 1) as usize;
        return data
            .get(idx)
            .cloned()
            .unwrap_or(FormulaValue::Error(FormulaError::Ref));
    }
    let r = (row_num - 1) as usize;
    let c = (col_num - 1) as usize;
    let idx = r * cols as usize + c;
    data.get(idx)
        .cloned()
        .unwrap_or(FormulaValue::Error(FormulaError::Ref))
}

fn eval_match(args: &[FormulaValue]) -> FormulaValue {
    if args.len() < 2 {
        return FormulaValue::Error(FormulaError::Value);
    }
    let lookup = &args[0];
    let lookup_array = flatten_args(&args[1..2]);
    let match_type = match args.get(2) {
        None => 1,
        Some(FormulaValue::Number(n)) => *n as i32,
        Some(FormulaValue::Error(e)) => return FormulaValue::Error(e.clone()),
        _ => return FormulaValue::Error(FormulaError::Value),
    };

    if match_type == 0 {
        for (i, val) in lookup_array.iter().enumerate() {
            if values_equal(val, lookup) {
                return FormulaValue::Number((i + 1) as f64);
            }
        }
        return FormulaValue::Error(FormulaError::Na);
    }

    if match_type == 1 {
        let mut last_match: Option<usize> = None;
        for (i, val) in lookup_array.iter().enumerate() {
            if vlookup_less_or_equal(val, lookup) {
                last_match = Some(i);
            } else {
                break;
            }
        }
        return last_match
            .map(|i| FormulaValue::Number((i + 1) as f64))
            .unwrap_or(FormulaValue::Error(FormulaError::Na));
    }

    if match_type == -1 {
        for (i, val) in lookup_array.iter().enumerate() {
            if vlookup_greater_or_equal(val, lookup) {
                return FormulaValue::Number((i + 1) as f64);
            }
        }
        return FormulaValue::Error(FormulaError::Na);
    }

    FormulaValue::Error(FormulaError::Value)
}

fn eval_sumifs(args: &[FormulaValue]) -> FormulaValue {
    if args.len() < 3 || args.len().is_multiple_of(2) {
        return FormulaValue::Error(FormulaError::Value);
    }
    let sum_range = flatten_args(&args[0..1]);
    let pairs = &args[1..];
    let mut sum = 0.0;
    for (i, val) in sum_range.iter().enumerate() {
        let mut all_match = true;
        for chunk in pairs.chunks(2) {
            let crit_range = flatten_args(&chunk[0..1]);
            let criterion = match criterion_string(&chunk[1]) {
                Some(s) => s,
                None => return FormulaValue::Error(FormulaError::Value),
            };
            let crit_val = crit_range.get(i).unwrap_or(&FormulaValue::Empty);
            if !matches_criterion(crit_val, &criterion) {
                all_match = false;
                break;
            }
        }
        if all_match {
            if let FormulaValue::Number(n) = val {
                sum += *n;
            }
        }
    }
    FormulaValue::Number(sum)
}

fn range_values_and_shape(
    value: &FormulaValue,
) -> Result<(Vec<FormulaValue>, u32, u32), FormulaError> {
    match value {
        FormulaValue::Array(data, rows, cols) => {
            let expected_len = (*rows as usize)
                .checked_mul(*cols as usize)
                .ok_or(FormulaError::Value)?;
            if *rows == 0 || *cols == 0 || expected_len != data.len() {
                return Err(FormulaError::Value);
            }
            Ok((data.clone(), *rows, *cols))
        }
        scalar => Ok((vec![scalar.clone()], 1, 1)),
    }
}

fn sumproduct_number(value: &FormulaValue) -> Result<f64, FormulaError> {
    match value {
        FormulaValue::Number(number) if number.is_finite() => Ok(*number),
        FormulaValue::Number(_) => Err(FormulaError::Value),
        FormulaValue::Boolean(true) => Ok(1.0),
        FormulaValue::Boolean(false) | FormulaValue::Empty => Ok(0.0),
        FormulaValue::String(_) => Ok(0.0),
        FormulaValue::Error(error) => Err(error.clone()),
        FormulaValue::Array(..) => Err(FormulaError::Value),
    }
}

fn eval_sumproduct(args: &[FormulaValue]) -> FormulaValue {
    if args.is_empty() {
        return FormulaValue::Error(FormulaError::Value);
    }

    let mut normalized = Vec::with_capacity(args.len());
    let mut array_shape: Option<(u32, u32)> = None;
    for arg in args {
        let (values, rows, cols) = match range_values_and_shape(arg) {
            Ok(range) => range,
            Err(error) => return FormulaValue::Error(error),
        };
        if matches!(arg, FormulaValue::Array(..)) {
            if let Some((expected_rows, expected_cols)) = array_shape {
                if (rows, cols) != (expected_rows, expected_cols) {
                    return FormulaValue::Error(FormulaError::Value);
                }
            } else {
                array_shape = Some((rows, cols));
            }
        }
        normalized.push((values, matches!(arg, FormulaValue::Array(..))));
    }

    let len = array_shape
        .map(|(rows, cols)| (rows as usize).saturating_mul(cols as usize))
        .unwrap_or(1);
    let mut total = 0.0;
    for index in 0..len {
        let mut product = 1.0;
        for (values, is_array) in &normalized {
            let value = if *is_array {
                values.get(index).unwrap_or(&FormulaValue::Empty)
            } else {
                &values[0]
            };
            let number = match sumproduct_number(value) {
                Ok(number) => number,
                Err(error) => return FormulaValue::Error(error),
            };
            product *= number;
            if !product.is_finite() {
                return FormulaValue::Error(FormulaError::Value);
            }
        }
        total += product;
        if !total.is_finite() {
            return FormulaValue::Error(FormulaError::Value);
        }
    }
    FormulaValue::Number(total)
}

fn eval_ifs_extreme(args: &[FormulaValue], minimum: bool) -> FormulaValue {
    if args.len() < 3 || args.len().is_multiple_of(2) {
        return FormulaValue::Error(FormulaError::Value);
    }
    let (target, target_rows, target_cols) = match range_values_and_shape(&args[0]) {
        Ok(range) => range,
        Err(error) => return FormulaValue::Error(error),
    };
    let mut criteria = Vec::with_capacity((args.len() - 1) / 2);
    for pair in args[1..].chunks(2) {
        let (values, rows, cols) = match range_values_and_shape(&pair[0]) {
            Ok(range) => range,
            Err(error) => return FormulaValue::Error(error),
        };
        if (rows, cols) != (target_rows, target_cols) {
            return FormulaValue::Error(FormulaError::Value);
        }
        let criterion = match criterion_string(&pair[1]) {
            Some(value) => value,
            None => {
                if let FormulaValue::Error(error) = &pair[1] {
                    return FormulaValue::Error(error.clone());
                }
                return FormulaValue::Error(FormulaError::Value);
            }
        };
        criteria.push((values, criterion));
    }

    let mut extreme: Option<f64> = None;
    for (index, value) in target.iter().enumerate() {
        if criteria.iter().all(|(values, criterion)| {
            matches_criterion(values.get(index).unwrap_or(&FormulaValue::Empty), criterion)
        }) {
            let number = match value {
                FormulaValue::Number(number) if number.is_finite() => *number,
                FormulaValue::Number(_) => return FormulaValue::Error(FormulaError::Value),
                FormulaValue::Error(error) => return FormulaValue::Error(error.clone()),
                _ => continue,
            };
            extreme = Some(match extreme {
                Some(current) if minimum => current.min(number),
                Some(current) => current.max(number),
                None => number,
            });
        }
    }
    FormulaValue::Number(extreme.unwrap_or(0.0))
}

fn eval_maxifs(args: &[FormulaValue]) -> FormulaValue {
    eval_ifs_extreme(args, false)
}

fn eval_minifs(args: &[FormulaValue]) -> FormulaValue {
    eval_ifs_extreme(args, true)
}

fn eval_countifs(args: &[FormulaValue]) -> FormulaValue {
    if args.len() < 2 || !args.len().is_multiple_of(2) {
        return FormulaValue::Error(FormulaError::Value);
    }
    let first_range = flatten_args(&args[0..1]);
    let pairs = args;
    let mut count = 0usize;
    for i in 0..first_range.len() {
        let mut all_match = true;
        for chunk in pairs.chunks(2) {
            let crit_range = flatten_args(&chunk[0..1]);
            let criterion = match criterion_string(&chunk[1]) {
                Some(s) => s,
                None => return FormulaValue::Error(FormulaError::Value),
            };
            let crit_val = crit_range.get(i).unwrap_or(&FormulaValue::Empty);
            if !matches_criterion(crit_val, &criterion) {
                all_match = false;
                break;
            }
        }
        if all_match {
            count += 1;
        }
    }
    FormulaValue::Number(count as f64)
}

fn eval_averageif(args: &[FormulaValue]) -> FormulaValue {
    if args.len() < 2 {
        return FormulaValue::Error(FormulaError::Value);
    }
    let range = flatten_args(&args[0..1]);
    let criterion = match criterion_string(&args[1]) {
        Some(s) => s,
        None => return FormulaValue::Error(FormulaError::Value),
    };
    let avg_range = if args.len() >= 3 {
        flatten_args(&args[2..3])
    } else {
        range.clone()
    };
    let mut sum = 0.0;
    let mut count = 0usize;
    for (i, val) in range.iter().enumerate() {
        if matches_criterion(val, &criterion) {
            if let Some(FormulaValue::Number(n)) = avg_range.get(i) {
                sum += *n;
                count += 1;
            }
        }
    }
    if count == 0 {
        FormulaValue::Error(FormulaError::DivZero)
    } else {
        FormulaValue::Number(sum / count as f64)
    }
}

fn eval_averageifs(args: &[FormulaValue]) -> FormulaValue {
    if args.len() < 3 || args.len().is_multiple_of(2) {
        return FormulaValue::Error(FormulaError::Value);
    }
    let avg_range = flatten_args(&args[0..1]);
    let pairs = &args[1..];
    let mut sum = 0.0;
    let mut count = 0usize;
    for (i, val) in avg_range.iter().enumerate() {
        let mut all_match = true;
        for chunk in pairs.chunks(2) {
            let crit_range = flatten_args(&chunk[0..1]);
            let criterion = match criterion_string(&chunk[1]) {
                Some(s) => s,
                None => return FormulaValue::Error(FormulaError::Value),
            };
            let crit_val = crit_range.get(i).unwrap_or(&FormulaValue::Empty);
            if !matches_criterion(crit_val, &criterion) {
                all_match = false;
                break;
            }
        }
        if all_match {
            if let FormulaValue::Number(n) = val {
                sum += *n;
                count += 1;
            }
        }
    }
    if count == 0 {
        FormulaValue::Error(FormulaError::DivZero)
    } else {
        FormulaValue::Number(sum / count as f64)
    }
}

fn eval_date(args: &[FormulaValue]) -> FormulaValue {
    if args.len() < 3 {
        return FormulaValue::Error(FormulaError::Value);
    }
    let year = match as_number_arg(&args[0]) {
        Some(n) => n as i32,
        None => return FormulaValue::Error(FormulaError::Value),
    };
    let month = match as_number_arg(&args[1]) {
        Some(n) => n as i32,
        None => return FormulaValue::Error(FormulaError::Value),
    };
    let day = match as_number_arg(&args[2]) {
        Some(n) => n as i32,
        None => return FormulaValue::Error(FormulaError::Value),
    };
    match excel_serial_from_ymd(year, month, day) {
        Some(serial) => FormulaValue::Number(serial),
        None => FormulaValue::Error(FormulaError::Value),
    }
}

fn eval_datevalue(args: &[FormulaValue]) -> FormulaValue {
    if args.is_empty() {
        return FormulaValue::Error(FormulaError::Value);
    }
    let text = match &args[0] {
        FormulaValue::String(s) => s.trim(),
        FormulaValue::Number(n) => return FormulaValue::Number(*n),
        FormulaValue::Error(e) => return FormulaValue::Error(e.clone()),
        _ => return FormulaValue::Error(FormulaError::Value),
    };
    if let Ok(serial) = text.parse::<f64>() {
        return FormulaValue::Number(serial);
    }
    parse_date_text(text)
        .and_then(|(y, m, d)| excel_serial_from_ymd(y, m, d))
        .map(FormulaValue::Number)
        .unwrap_or(FormulaValue::Error(FormulaError::Value))
}

fn date_number(value: &FormulaValue) -> Result<f64, FormulaError> {
    match value {
        FormulaValue::Number(number) if number.is_finite() => Ok(*number),
        FormulaValue::Number(_) => Err(FormulaError::Value),
        FormulaValue::Error(error) => Err(error.clone()),
        _ => Err(FormulaError::Value),
    }
}

fn eval_time(args: &[FormulaValue]) -> FormulaValue {
    if args.len() != 3 {
        return FormulaValue::Error(FormulaError::Value);
    }
    let hour = match date_number(&args[0]) {
        Ok(value) => value,
        Err(error) => return FormulaValue::Error(error),
    };
    let minute = match date_number(&args[1]) {
        Ok(value) => value,
        Err(error) => return FormulaValue::Error(error),
    };
    let second = match date_number(&args[2]) {
        Ok(value) => value,
        Err(error) => return FormulaValue::Error(error),
    };
    if hour < 0.0 || minute < 0.0 || second < 0.0 {
        return FormulaValue::Error(FormulaError::Value);
    }
    let seconds = hour * 3600.0 + minute * 60.0 + second;
    let fraction = (seconds / 86_400.0).rem_euclid(1.0);
    if fraction.is_finite() {
        FormulaValue::Number(fraction)
    } else {
        FormulaValue::Error(FormulaError::Value)
    }
}

fn time_component(args: &[FormulaValue], component: fn(f64) -> i64) -> FormulaValue {
    if args.len() != 1 {
        return FormulaValue::Error(FormulaError::Value);
    }
    let serial = match date_number(&args[0]) {
        Ok(value) if value >= 0.0 => value,
        Ok(_) => return FormulaValue::Error(FormulaError::Value),
        Err(error) => return FormulaValue::Error(error),
    };
    let fraction = serial.fract();
    FormulaValue::Number(component(fraction) as f64)
}

fn eval_hour(args: &[FormulaValue]) -> FormulaValue {
    time_component(args, |fraction| (fraction * 24.0).floor() as i64)
}

fn eval_minute(args: &[FormulaValue]) -> FormulaValue {
    time_component(args, |fraction| {
        ((fraction * 1_440.0).floor() as i64).rem_euclid(60)
    })
}

fn eval_second(args: &[FormulaValue]) -> FormulaValue {
    time_component(args, |fraction| {
        ((fraction * 86_400.0).floor() as i64).rem_euclid(60)
    })
}

fn weekday_sunday_index(days_since_unix_epoch: i64) -> i64 {
    // 1970-01-01 was Thursday (Sunday-based index 4).
    (days_since_unix_epoch + 4).rem_euclid(7)
}

fn eval_weekday(args: &[FormulaValue]) -> FormulaValue {
    if !(1..=2).contains(&args.len()) {
        return FormulaValue::Error(FormulaError::Value);
    }
    let serial = match date_number(&args[0]) {
        Ok(value) if value >= 0.0 => value,
        Ok(_) => return FormulaValue::Error(FormulaError::Value),
        Err(error) => return FormulaValue::Error(error),
    };
    let (year, month, day) = match excel_ymd_from_serial(serial) {
        Some(value) => value,
        None => return FormulaValue::Error(FormulaError::Value),
    };
    let days = match civil_to_days(year, month, day) {
        Some(value) => value,
        None => return FormulaValue::Error(FormulaError::Value),
    };
    let sunday_index = weekday_sunday_index(days);
    let return_type = match args.get(1) {
        None => 1,
        Some(FormulaValue::Number(value))
            if value.is_finite()
                && value.fract() == 0.0
                && *value >= i32::MIN as f64
                && *value <= i32::MAX as f64 =>
        {
            *value as i32
        }
        Some(FormulaValue::Error(error)) => return FormulaValue::Error(error.clone()),
        Some(_) => return FormulaValue::Error(FormulaError::Value),
    };
    let result = match return_type {
        1 => sunday_index + 1,
        2 => (sunday_index + 6).rem_euclid(7) + 1,
        3 => (sunday_index + 6).rem_euclid(7),
        11..=17 => (sunday_index - i64::from(return_type - 11)).rem_euclid(7) + 1,
        _ => return FormulaValue::Error(FormulaError::Value),
    };
    FormulaValue::Number(result as f64)
}

fn eval_networkdays(args: &[FormulaValue]) -> FormulaValue {
    if !(2..=3).contains(&args.len()) {
        return FormulaValue::Error(FormulaError::Value);
    }
    let start_serial = match date_number(&args[0]) {
        Ok(value) => value,
        Err(error) => return FormulaValue::Error(error),
    };
    let end_serial = match date_number(&args[1]) {
        Ok(value) => value,
        Err(error) => return FormulaValue::Error(error),
    };
    if start_serial < 0.0 || end_serial < 0.0 {
        return FormulaValue::Error(FormulaError::Value);
    }
    let start = match excel_ymd_from_serial(start_serial.floor())
        .and_then(|(year, month, day)| civil_to_days(year, month, day))
    {
        Some(value) => value,
        None => return FormulaValue::Error(FormulaError::Value),
    };
    let end = match excel_ymd_from_serial(end_serial.floor())
        .and_then(|(year, month, day)| civil_to_days(year, month, day))
    {
        Some(value) => value,
        None => return FormulaValue::Error(FormulaError::Value),
    };
    let (first, last, sign) = if start <= end {
        (start, end, 1.0)
    } else {
        (end, start, -1.0)
    };
    if last.saturating_sub(first) > 1_000_000 {
        return FormulaValue::Error(FormulaError::Value);
    }

    let mut holidays = HashSet::new();
    if let Some(FormulaValue::Array(values, _, _)) = args.get(2) {
        for value in values {
            match value {
                FormulaValue::Number(serial) if serial.is_finite() && *serial >= 0.0 => {
                    if let Some((year, month, day)) = excel_ymd_from_serial(serial.floor()) {
                        if let Some(holiday) = civil_to_days(year, month, day) {
                            holidays.insert(holiday);
                        }
                    }
                }
                FormulaValue::Number(_) => return FormulaValue::Error(FormulaError::Value),
                FormulaValue::Error(error) => return FormulaValue::Error(error.clone()),
                _ => {}
            }
        }
    } else if let Some(value) = args.get(2) {
        match value {
            FormulaValue::Number(serial) if serial.is_finite() && *serial >= 0.0 => {
                if let Some((year, month, day)) = excel_ymd_from_serial(serial.floor()) {
                    if let Some(holiday) = civil_to_days(year, month, day) {
                        holidays.insert(holiday);
                    }
                }
            }
            FormulaValue::Number(_) => return FormulaValue::Error(FormulaError::Value),
            FormulaValue::Error(error) => return FormulaValue::Error(error.clone()),
            FormulaValue::Empty => {}
            _ => return FormulaValue::Error(FormulaError::Value),
        }
    }

    let mut weekdays = 0i64;
    for day in first..=last {
        let sunday_index = weekday_sunday_index(day);
        if sunday_index > 0 && sunday_index < 6 && !holidays.contains(&day) {
            weekdays += 1;
        }
    }
    FormulaValue::Number(weekdays as f64 * sign)
}

fn eval_days(args: &[FormulaValue]) -> FormulaValue {
    if args.len() != 2 {
        return FormulaValue::Error(FormulaError::Value);
    }
    let end = match date_number(&args[0]) {
        Ok(value) if value >= 0.0 => value.floor(),
        Ok(_) => return FormulaValue::Error(FormulaError::Value),
        Err(error) => return FormulaValue::Error(error),
    };
    let start = match date_number(&args[1]) {
        Ok(value) if value >= 0.0 => value.floor(),
        Ok(_) => return FormulaValue::Error(FormulaError::Value),
        Err(error) => return FormulaValue::Error(error),
    };
    let result = end - start;
    if result.is_finite() {
        FormulaValue::Number(result)
    } else {
        FormulaValue::Error(FormulaError::Value)
    }
}

fn eval_edate(args: &[FormulaValue]) -> FormulaValue {
    if args.len() != 2 {
        return FormulaValue::Error(FormulaError::Value);
    }
    let serial = match date_number(&args[0]) {
        Ok(value) if value >= 0.0 => value,
        Ok(_) => return FormulaValue::Error(FormulaError::Value),
        Err(error) => return FormulaValue::Error(error),
    };
    let months = match date_number(&args[1]) {
        Ok(value) if value.is_finite() && value.trunc() == value && value.abs() <= 120_000.0 => {
            value as i64
        }
        Ok(value) if value.is_finite() && value.abs() <= 120_000.0 => value.trunc() as i64,
        Ok(_) => return FormulaValue::Error(FormulaError::Value),
        Err(error) => return FormulaValue::Error(error),
    };
    let (year, month, day) = match excel_ymd_from_serial(serial) {
        Some(value) => value,
        None => return FormulaValue::Error(FormulaError::Value),
    };
    let total_months = i64::from(year)
        .checked_mul(12)
        .and_then(|value| value.checked_add(i64::from(month - 1)))
        .and_then(|value| value.checked_add(months));
    let total_months = match total_months {
        Some(value) => value,
        None => return FormulaValue::Error(FormulaError::Value),
    };
    let target_year = total_months.div_euclid(12);
    if !(i32::MIN as i64..=i32::MAX as i64).contains(&target_year) {
        return FormulaValue::Error(FormulaError::Value);
    }
    let target_month = total_months.rem_euclid(12) as i32 + 1;
    let target_day = day.min(days_in_month(target_year as i32, target_month));
    match excel_serial_from_ymd(target_year as i32, target_month, target_day) {
        Some(result) if result.is_finite() && result >= 0.0 => FormulaValue::Number(result),
        _ => FormulaValue::Error(FormulaError::Value),
    }
}

fn parse_holiday_days(value: Option<&FormulaValue>) -> Result<HashSet<i64>, FormulaError> {
    let mut holidays = HashSet::new();
    let Some(value) = value else {
        return Ok(holidays);
    };
    let values: Vec<&FormulaValue> = match value {
        FormulaValue::Array(values, _, _) => values.iter().collect(),
        value => vec![value],
    };
    for value in values {
        match value {
            FormulaValue::Number(serial) if serial.is_finite() && *serial >= 0.0 => {
                let Some((year, month, day)) = excel_ymd_from_serial(serial.floor()) else {
                    return Err(FormulaError::Value);
                };
                let Some(day) = civil_to_days(year, month, day) else {
                    return Err(FormulaError::Value);
                };
                holidays.insert(day);
            }
            FormulaValue::Number(_) => return Err(FormulaError::Value),
            FormulaValue::Error(error) => return Err(error.clone()),
            FormulaValue::Empty => {}
            _ => return Err(FormulaError::Value),
        }
    }
    Ok(holidays)
}

fn eval_workday(args: &[FormulaValue]) -> FormulaValue {
    if !(2..=3).contains(&args.len()) {
        return FormulaValue::Error(FormulaError::Value);
    }
    let start_serial = match date_number(&args[0]) {
        Ok(value) if value >= 0.0 => value,
        Ok(_) => return FormulaValue::Error(FormulaError::Value),
        Err(error) => return FormulaValue::Error(error),
    };
    let requested_days = match date_number(&args[1]) {
        Ok(value) if value.is_finite() && value.abs() <= 1_000_000.0 => value.trunc() as i64,
        Ok(_) => return FormulaValue::Error(FormulaError::Value),
        Err(error) => return FormulaValue::Error(error),
    };
    let holidays = match parse_holiday_days(args.get(2)) {
        Ok(value) => value,
        Err(error) => return FormulaValue::Error(error),
    };
    let (year, month, day) = match excel_ymd_from_serial(start_serial.floor()) {
        Some(value) => value,
        None => return FormulaValue::Error(FormulaError::Value),
    };
    let epoch = match civil_to_days(EXCEL_EPOCH_YEAR, EXCEL_EPOCH_MONTH, EXCEL_EPOCH_DAY) {
        Some(value) => value,
        None => return FormulaValue::Error(FormulaError::Value),
    };
    let mut current = match civil_to_days(year, month, day) {
        Some(value) => value,
        None => return FormulaValue::Error(FormulaError::Value),
    };
    let direction = if requested_days < 0 { -1_i64 } else { 1_i64 };
    let mut remaining = requested_days.unsigned_abs();
    let mut steps = 0_u64;
    while remaining > 0 || (requested_days == 0 && !is_workday(current, &holidays)) {
        current = match current.checked_add(direction) {
            Some(value) => value,
            None => return FormulaValue::Error(FormulaError::Value),
        };
        steps += 1;
        if steps > 4_000_000 {
            return FormulaValue::Error(FormulaError::Value);
        }
        if is_workday(current, &holidays) {
            remaining = remaining.saturating_sub(1);
        }
    }
    let serial = current - epoch;
    if serial >= 0 {
        FormulaValue::Number(serial as f64)
    } else {
        FormulaValue::Error(FormulaError::Value)
    }
}

fn is_workday(day: i64, holidays: &HashSet<i64>) -> bool {
    let sunday_index = weekday_sunday_index(day);
    sunday_index > 0 && sunday_index < 6 && !holidays.contains(&day)
}

fn parse_date_text(text: &str) -> Option<(i32, i32, i32)> {
    let parts: Vec<&str> = text
        .split(&['/', '-', '.', ' '][..])
        .filter(|p| !p.is_empty())
        .collect();
    if parts.len() < 3 {
        return None;
    }
    let a = parts[0].parse::<i32>().ok()?;
    let b = parts[1].parse::<i32>().ok()?;
    let c = parts[2].parse::<i32>().ok()?;
    if a > 31 {
        Some((a, b, c))
    } else {
        Some((c, a, b))
    }
}

fn eval_year(args: &[FormulaValue]) -> FormulaValue {
    extract_date_component(args, |y, _, _| y)
}

fn eval_month(args: &[FormulaValue]) -> FormulaValue {
    extract_date_component(args, |_, m, _| m)
}

fn eval_day(args: &[FormulaValue]) -> FormulaValue {
    extract_date_component(args, |_, _, d| d)
}

fn extract_date_component(
    args: &[FormulaValue],
    pick: impl FnOnce(i32, i32, i32) -> i32,
) -> FormulaValue {
    if args.is_empty() {
        return FormulaValue::Error(FormulaError::Value);
    }
    let serial = match &args[0] {
        FormulaValue::Number(n) => *n,
        FormulaValue::Error(e) => return FormulaValue::Error(e.clone()),
        _ => return FormulaValue::Error(FormulaError::Value),
    };
    match excel_ymd_from_serial(serial) {
        Some((y, m, d)) => FormulaValue::Number(pick(y, m, d) as f64),
        None => FormulaValue::Error(FormulaError::Value),
    }
}

fn eval_eomonth(args: &[FormulaValue]) -> FormulaValue {
    if args.len() < 2 {
        return FormulaValue::Error(FormulaError::Value);
    }
    let serial = match as_number_arg(&args[0]) {
        Some(n) => n,
        None => return FormulaValue::Error(FormulaError::Value),
    };
    let months = match as_number_arg(&args[1]) {
        Some(n) => n as i32,
        None => return FormulaValue::Error(FormulaError::Value),
    };
    let (year, month, _) = match excel_ymd_from_serial(serial) {
        Some(v) => v,
        None => return FormulaValue::Error(FormulaError::Value),
    };
    let mut y = year;
    let mut m = month + months;
    while m > 12 {
        m -= 12;
        y += 1;
    }
    while m < 1 {
        m += 12;
        y -= 1;
    }
    let day = days_in_month(y, m);
    match excel_serial_from_ymd(y, m, day) {
        Some(s) => FormulaValue::Number(s),
        None => FormulaValue::Error(FormulaError::Value),
    }
}

fn eval_isblank(args: &[FormulaValue]) -> FormulaValue {
    if args.is_empty() {
        return FormulaValue::Error(FormulaError::Value);
    }
    match &args[0] {
        FormulaValue::Empty => FormulaValue::Boolean(true),
        FormulaValue::String(s) if s.is_empty() => FormulaValue::Boolean(true),
        FormulaValue::Error(e) => FormulaValue::Error(e.clone()),
        _ => FormulaValue::Boolean(false),
    }
}

fn eval_iserror(args: &[FormulaValue]) -> FormulaValue {
    if args.is_empty() {
        return FormulaValue::Error(FormulaError::Value);
    }
    FormulaValue::Boolean(matches!(args[0], FormulaValue::Error(_)))
}

fn eval_isnumber(args: &[FormulaValue]) -> FormulaValue {
    if args.is_empty() {
        return FormulaValue::Error(FormulaError::Value);
    }
    FormulaValue::Boolean(matches!(args[0], FormulaValue::Number(_)))
}

fn eval_istext(args: &[FormulaValue]) -> FormulaValue {
    if args.is_empty() {
        return FormulaValue::Error(FormulaError::Value);
    }
    FormulaValue::Boolean(matches!(args[0], FormulaValue::String(_)))
}

fn eval_islogical(args: &[FormulaValue]) -> FormulaValue {
    if args.is_empty() {
        return FormulaValue::Error(FormulaError::Value);
    }
    FormulaValue::Boolean(matches!(args[0], FormulaValue::Boolean(_)))
}

fn eval_value(args: &[FormulaValue]) -> FormulaValue {
    if args.is_empty() {
        return FormulaValue::Error(FormulaError::Value);
    }
    match &args[0] {
        FormulaValue::Number(n) => FormulaValue::Number(*n),
        FormulaValue::String(s) => {
            let trimmed = s.trim().replace(',', "");
            if trimmed.is_empty() {
                return FormulaValue::Number(0.0);
            }
            if let Ok(n) = trimmed.parse::<f64>() {
                FormulaValue::Number(n)
            } else {
                FormulaValue::Error(FormulaError::Value)
            }
        }
        FormulaValue::Boolean(b) => FormulaValue::Number(if *b { 1.0 } else { 0.0 }),
        FormulaValue::Error(e) => FormulaValue::Error(e.clone()),
        FormulaValue::Empty => FormulaValue::Number(0.0),
        FormulaValue::Array(..) => FormulaValue::Error(FormulaError::Value),
    }
}

fn eval_substitute(args: &[FormulaValue]) -> FormulaValue {
    if args.len() < 3 {
        return FormulaValue::Error(FormulaError::Value);
    }
    let text = match string_arg(&args[0]) {
        Some(s) => s,
        None => return FormulaValue::Error(FormulaError::Value),
    };
    let old = match string_arg(&args[1]) {
        Some(s) => s,
        None => return FormulaValue::Error(FormulaError::Value),
    };
    let new = match string_arg(&args[2]) {
        Some(s) => s,
        None => return FormulaValue::Error(FormulaError::Value),
    };
    if old.is_empty() {
        return FormulaValue::Error(FormulaError::Value);
    }
    if let Some(FormulaValue::Number(inst)) = args.get(3) {
        let instance = *inst as usize;
        if instance == 0 {
            return FormulaValue::Error(FormulaError::Value);
        }
        let mut count = 0usize;
        let mut result = String::new();
        let mut rest = text.as_str();
        while let Some(pos) = rest.find(&old) {
            count += 1;
            if count == instance {
                result.push_str(&rest[..pos]);
                result.push_str(&new);
                result.push_str(&rest[pos + old.len()..]);
                return FormulaValue::String(result);
            }
            result.push_str(&rest[..pos + old.len()]);
            rest = &rest[pos + old.len()..];
        }
        return FormulaValue::String(text);
    }
    FormulaValue::String(text.replace(&old, &new))
}

fn eval_replace(args: &[FormulaValue]) -> FormulaValue {
    if args.len() < 4 {
        return FormulaValue::Error(FormulaError::Value);
    }
    let old_text = match string_arg(&args[0]) {
        Some(s) => s,
        None => return FormulaValue::Error(FormulaError::Value),
    };
    let start = match as_number_arg(&args[1]) {
        Some(n) => n as usize,
        None => return FormulaValue::Error(FormulaError::Value),
    };
    let num_chars = match as_number_arg(&args[2]) {
        Some(n) => n as usize,
        None => return FormulaValue::Error(FormulaError::Value),
    };
    let new_text = match string_arg(&args[3]) {
        Some(s) => s,
        None => return FormulaValue::Error(FormulaError::Value),
    };
    if start < 1 {
        return FormulaValue::Error(FormulaError::Value);
    }
    let chars: Vec<char> = old_text.chars().collect();
    let start_idx = start - 1;
    if start_idx > chars.len() {
        return FormulaValue::Error(FormulaError::Value);
    }
    let end_idx = (start_idx + num_chars).min(chars.len());
    let mut result = String::new();
    result.extend(chars[..start_idx].iter());
    result.push_str(&new_text);
    result.extend(chars[end_idx..].iter());
    FormulaValue::String(result)
}

fn eval_find(args: &[FormulaValue], case_sensitive: bool) -> FormulaValue {
    if args.len() < 2 {
        return FormulaValue::Error(FormulaError::Value);
    }
    let find_text = match string_arg(&args[0]) {
        Some(s) => s,
        None => return FormulaValue::Error(FormulaError::Value),
    };
    let within_text = match string_arg(&args[1]) {
        Some(s) => s,
        None => return FormulaValue::Error(FormulaError::Value),
    };
    let start = match args.get(2) {
        None => 1,
        Some(FormulaValue::Number(n)) => *n as usize,
        Some(FormulaValue::Error(e)) => return FormulaValue::Error(e.clone()),
        _ => return FormulaValue::Error(FormulaError::Value),
    };
    if start < 1 || find_text.is_empty() {
        return FormulaValue::Error(FormulaError::Value);
    }
    let within_chars: Vec<char> = within_text.chars().collect();
    let start_idx = start - 1;
    if start_idx >= within_chars.len() {
        return FormulaValue::Error(FormulaError::Value);
    }
    let slice: String = within_chars[start_idx..].iter().collect();
    let pos = if case_sensitive {
        slice.find(&find_text)
    } else {
        slice
            .to_ascii_lowercase()
            .find(&find_text.to_ascii_lowercase())
    };
    match pos {
        Some(p) => FormulaValue::Number((start_idx + p + 1) as f64),
        None => FormulaValue::Error(FormulaError::Value),
    }
}

fn eval_randbetween(args: &[FormulaValue]) -> FormulaValue {
    if args.len() < 2 {
        return FormulaValue::Error(FormulaError::Value);
    }
    let bottom = match as_number_arg(&args[0]) {
        Some(n) => n.floor() as i64,
        None => return FormulaValue::Error(FormulaError::Value),
    };
    let top = match as_number_arg(&args[1]) {
        Some(n) => n.floor() as i64,
        None => return FormulaValue::Error(FormulaError::Value),
    };
    if bottom > top {
        return FormulaValue::Error(FormulaError::Value);
    }
    let span = (top - bottom + 1) as u64;
    let offset = (rand_unit() * span as f64).floor() as u64;
    FormulaValue::Number((bottom + offset as i64) as f64)
}

fn eval_hlookup(args: &[FormulaValue]) -> FormulaValue {
    if args.len() < 3 {
        return FormulaValue::Error(FormulaError::Value);
    }
    let lookup = match &args[0] {
        FormulaValue::Array(data, _, _) => data.first().cloned().unwrap_or(FormulaValue::Empty),
        v => v.clone(),
    };
    let row_index = match as_number_arg(&args[2]) {
        Some(n) => n as usize,
        None => return FormulaValue::Error(FormulaError::Value),
    };
    if row_index < 1 {
        return FormulaValue::Error(FormulaError::Value);
    }
    let range_lookup = match args.get(3) {
        None => true,
        Some(FormulaValue::Boolean(b)) => *b,
        Some(FormulaValue::Number(n)) => *n != 0.0,
        Some(FormulaValue::Error(e)) => return FormulaValue::Error(e.clone()),
        _ => true,
    };
    let (table_data, rows, cols) = match &args[1] {
        FormulaValue::Array(data, r, c) => (data.as_slice(), *r as usize, *c as usize),
        val => (std::slice::from_ref(val), 1, 1),
    };
    if table_data.is_empty() || rows < row_index || cols == 0 {
        return FormulaValue::Error(FormulaError::Ref);
    }
    let header = &table_data[..cols];
    if !range_lookup {
        for (i, val) in header.iter().enumerate() {
            if values_equal(val, &lookup) {
                let idx = (row_index - 1) * cols + i;
                return table_data
                    .get(idx)
                    .cloned()
                    .unwrap_or(FormulaValue::Error(FormulaError::Ref));
            }
        }
        return FormulaValue::Error(FormulaError::Na);
    }
    let mut last_match: Option<usize> = None;
    for (i, val) in header.iter().enumerate() {
        if values_equal(val, &lookup) {
            let idx = (row_index - 1) * cols + i;
            return table_data
                .get(idx)
                .cloned()
                .unwrap_or(FormulaValue::Error(FormulaError::Ref));
        }
        if vlookup_less_or_equal(val, &lookup) {
            last_match = Some(i);
        } else {
            break;
        }
    }
    if let Some(i) = last_match {
        let idx = (row_index - 1) * cols + i;
        return table_data
            .get(idx)
            .cloned()
            .unwrap_or(FormulaValue::Error(FormulaError::Ref));
    }
    FormulaValue::Error(FormulaError::Na)
}

fn eval_choose(args: &[FormulaValue]) -> FormulaValue {
    if args.len() < 2 {
        return FormulaValue::Error(FormulaError::Value);
    }
    let index = match as_number_arg(&args[0]) {
        Some(n) => n as usize,
        None => return FormulaValue::Error(FormulaError::Value),
    };
    if index < 1 || index > args.len() - 1 {
        return FormulaValue::Error(FormulaError::Value);
    }
    args[index].clone()
}

fn string_arg(val: &FormulaValue) -> Option<String> {
    match val {
        FormulaValue::String(s) => Some(s.clone()),
        FormulaValue::Number(n) => Some(n.to_string()),
        FormulaValue::Boolean(b) => Some(b.to_string()),
        FormulaValue::Error(e) => Some(e.to_str().to_string()),
        FormulaValue::Empty => Some(String::new()),
        FormulaValue::Array(..) => None,
    }
}

fn values_equal(a: &FormulaValue, b: &FormulaValue) -> bool {
    match (a, b) {
        (FormulaValue::Number(x), FormulaValue::Number(y)) => (x - y).abs() < 1e-9,
        (FormulaValue::String(x), FormulaValue::String(y)) => x.eq_ignore_ascii_case(y),
        (FormulaValue::Number(x), FormulaValue::String(y))
        | (FormulaValue::String(y), FormulaValue::Number(x)) => y
            .parse::<f64>()
            .map(|n| (n - *x).abs() < 1e-9)
            .unwrap_or(false),
        (FormulaValue::Boolean(x), FormulaValue::Boolean(y)) => x == y,
        _ => false,
    }
}

fn vlookup_less_or_equal(a: &FormulaValue, b: &FormulaValue) -> bool {
    match (a, b) {
        (FormulaValue::Number(x), FormulaValue::Number(y)) => x <= y,
        (FormulaValue::String(x), FormulaValue::String(y)) => x <= y,
        (FormulaValue::Number(x), FormulaValue::String(y)) => {
            y.parse::<f64>().map(|n| x <= &n).unwrap_or(false)
        }
        (FormulaValue::String(x), FormulaValue::Number(y)) => {
            x.parse::<f64>().map(|n| &n <= y).unwrap_or(false)
        }
        _ => false,
    }
}

fn vlookup_greater_or_equal(a: &FormulaValue, b: &FormulaValue) -> bool {
    match (a, b) {
        (FormulaValue::Number(x), FormulaValue::Number(y)) => x >= y,
        (FormulaValue::String(x), FormulaValue::String(y)) => x >= y,
        (FormulaValue::Number(x), FormulaValue::String(y)) => {
            y.parse::<f64>().map(|n| x >= &n).unwrap_or(false)
        }
        (FormulaValue::String(x), FormulaValue::Number(y)) => {
            x.parse::<f64>().map(|n| &n >= y).unwrap_or(false)
        }
        _ => false,
    }
}

fn matches_criterion(value: &FormulaValue, criterion: &str) -> bool {
    let c = criterion.trim();
    if c.is_empty() {
        return match value {
            FormulaValue::Empty => true,
            FormulaValue::String(s) if s.is_empty() => true,
            _ => false,
        };
    }
    let (op, rhs) = if let Some(rest) = c.strip_prefix(">=") {
        (">=", rest)
    } else if let Some(rest) = c.strip_prefix("<=") {
        ("<=", rest)
    } else if let Some(rest) = c.strip_prefix("<>") {
        ("<>", rest)
    } else if let Some(rest) = c.strip_prefix('>') {
        (">", rest)
    } else if let Some(rest) = c.strip_prefix('<') {
        ("<", rest)
    } else if let Some(rest) = c.strip_prefix('=') {
        ("=", rest)
    } else {
        ("=", c)
    };
    match value {
        FormulaValue::Number(n) => {
            let Ok(target) = rhs.parse::<f64>() else {
                return false;
            };
            match op {
                ">=" => *n >= target,
                "<=" => *n <= target,
                ">" => *n > target,
                "<" => *n < target,
                "<>" => (*n - target).abs() >= 1e-9,
                _ => (*n - target).abs() < 1e-9,
            }
        }
        FormulaValue::String(s) => match op {
            "<>" => !s.eq_ignore_ascii_case(rhs),
            _ => s.eq_ignore_ascii_case(rhs),
        },
        FormulaValue::Boolean(b) => {
            let as_str = b.to_string();
            match op {
                "<>" => !as_str.eq_ignore_ascii_case(rhs),
                _ => as_str.eq_ignore_ascii_case(rhs),
            }
        }
        FormulaValue::Empty => rhs.is_empty() && op == "=",
        FormulaValue::Error(_) => false,
        FormulaValue::Array(..) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arr(values: &[FormulaValue], rows: u32, cols: u32) -> FormulaValue {
        FormulaValue::Array(values.to_vec(), rows, cols)
    }

    #[test]
    fn test_round_negative_and_large_digits() {
        let res_neg = eval_func(
            "ROUND",
            &[FormulaValue::Number(1234.56), FormulaValue::Number(-2.0)],
        );
        assert_eq!(res_neg, FormulaValue::Number(1200.0));

        let res_neg_one = eval_func(
            "ROUND",
            &[FormulaValue::Number(125.0), FormulaValue::Number(-1.0)],
        );
        assert_eq!(res_neg_one, FormulaValue::Number(130.0));

        let res_large_pos = eval_func(
            "ROUND",
            &[
                FormulaValue::Number(1.23456789),
                FormulaValue::Number(100.0),
            ],
        );
        assert_eq!(res_large_pos, FormulaValue::Number(1.23456789));

        let res_large_neg = eval_func(
            "ROUND",
            &[FormulaValue::Number(123456.0), FormulaValue::Number(-100.0)],
        );
        assert_eq!(res_large_neg, FormulaValue::Number(0.0));
    }

    #[test]
    fn test_bounded_math_compatibility_helpers() {
        let number = FormulaValue::Number(123.456);
        assert_eq!(
            eval_func("ROUNDUP", &[number.clone(), FormulaValue::Number(2.0)]),
            FormulaValue::Number(123.46)
        );
        assert_eq!(
            eval_func(
                "ROUNDDOWN",
                &[FormulaValue::Number(-123.456), FormulaValue::Number(2.0)],
            ),
            FormulaValue::Number(-123.45)
        );
        assert_eq!(
            eval_func("INT", &[FormulaValue::Number(-1.2)]),
            FormulaValue::Number(-2.0)
        );
        assert_eq!(
            eval_func(
                "TRUNC",
                &[FormulaValue::Number(-1.29), FormulaValue::Number(1.0)],
            ),
            FormulaValue::Number(-1.2)
        );
        assert_eq!(
            eval_func("SIGN", &[FormulaValue::Number(-4.0)]),
            FormulaValue::Number(-1.0)
        );
        assert_eq!(
            eval_func("SIGN", &[FormulaValue::Number(0.0)]),
            FormulaValue::Number(0.0)
        );

        assert_eq!(
            eval_func(
                "LOG",
                &[FormulaValue::Number(100.0), FormulaValue::Number(10.0)]
            ),
            FormulaValue::Number(2.0)
        );
        assert_eq!(
            eval_func("LOG10", &[FormulaValue::Number(100.0)]),
            FormulaValue::Number(2.0)
        );
        let ln_e = eval_func("LN", &[FormulaValue::Number(std::f64::consts::E)]);
        assert!(matches!(ln_e, FormulaValue::Number(value) if (value - 1.0).abs() < 1e-12));
        let exp_one = eval_func("EXP", &[FormulaValue::Number(1.0)]);
        assert!(
            matches!(exp_one, FormulaValue::Number(value) if (value - std::f64::consts::E).abs() < 1e-12)
        );
        let sin_half_pi = eval_func("SIN", &[FormulaValue::Number(std::f64::consts::FRAC_PI_2)]);
        assert!(matches!(sin_half_pi, FormulaValue::Number(value) if (value - 1.0).abs() < 1e-12));
        let cos_pi = eval_func("COS", &[FormulaValue::Number(std::f64::consts::PI)]);
        assert!(matches!(cos_pi, FormulaValue::Number(value) if (value + 1.0).abs() < 1e-12));

        assert!(matches!(
            eval_func("LOG", &[FormulaValue::Number(-1.0)]),
            FormulaValue::Error(FormulaError::Value)
        ));
        assert!(matches!(
            eval_func("ROUNDUP", &[number, FormulaValue::String("bad".into())]),
            FormulaValue::Error(FormulaError::Value)
        ));
    }

    #[test]
    fn test_xlookup_exact() {
        let lookup_range = arr(
            &[
                FormulaValue::Number(10.0),
                FormulaValue::Number(20.0),
                FormulaValue::Number(30.0),
            ],
            3,
            1,
        );
        let return_range = arr(
            &[
                FormulaValue::String("a".into()),
                FormulaValue::String("b".into()),
                FormulaValue::String("c".into()),
            ],
            3,
            1,
        );
        let res = eval_func(
            "XLOOKUP",
            &[FormulaValue::Number(20.0), lookup_range, return_range],
        );
        assert_eq!(res, FormulaValue::String("b".into()));

        let not_found = eval_func(
            "XLOOKUP",
            &[
                FormulaValue::Number(99.0),
                arr(&[FormulaValue::Number(1.0)], 1, 1),
                arr(&[FormulaValue::String("x".into())], 1, 1),
                FormulaValue::String("missing".into()),
            ],
        );
        assert_eq!(not_found, FormulaValue::String("missing".into()));
    }

    #[test]
    fn test_index() {
        let matrix = arr(
            &[
                FormulaValue::Number(1.0),
                FormulaValue::Number(2.0),
                FormulaValue::Number(3.0),
                FormulaValue::Number(4.0),
            ],
            2,
            2,
        );
        assert_eq!(
            eval_func("INDEX", &[matrix.clone(), FormulaValue::Number(2.0)]),
            FormulaValue::Number(3.0)
        );
        assert_eq!(
            eval_func(
                "INDEX",
                &[matrix, FormulaValue::Number(2.0), FormulaValue::Number(2.0)]
            ),
            FormulaValue::Number(4.0)
        );
        let row = arr(
            &[
                FormulaValue::Number(10.0),
                FormulaValue::Number(20.0),
                FormulaValue::Number(30.0),
            ],
            1,
            3,
        );
        assert_eq!(
            eval_func("INDEX", &[row.clone(), FormulaValue::Number(2.0)]),
            FormulaValue::Number(20.0)
        );
        assert_eq!(
            eval_func(
                "INDEX",
                &[row, FormulaValue::Number(1.0), FormulaValue::Number(2.0)]
            ),
            FormulaValue::Number(20.0)
        );
    }

    #[test]
    fn test_text_and_boolean_compatibility_helpers() {
        assert_eq!(
            eval_func(
                "CONCATENATE",
                &[
                    FormulaValue::String("Red".into()),
                    FormulaValue::String("oc".into()),
                    FormulaValue::Number(2.0),
                ],
            ),
            FormulaValue::String("Redoc2".into())
        );
        assert_eq!(
            eval_func(
                "COUNTBLANK",
                &[arr(
                    &[
                        FormulaValue::Empty,
                        FormulaValue::String(String::new()),
                        FormulaValue::String("value".into()),
                    ],
                    1,
                    3,
                )],
            ),
            FormulaValue::Number(2.0)
        );
        assert_eq!(
            eval_func(
                "XOR",
                &[
                    FormulaValue::Boolean(true),
                    FormulaValue::Number(1.0),
                    FormulaValue::Boolean(false),
                ],
            ),
            FormulaValue::Boolean(false)
        );
    }

    #[test]
    fn test_array_shape_functions() {
        let matrix = arr(
            &[
                FormulaValue::Number(1.0),
                FormulaValue::Number(2.0),
                FormulaValue::Number(3.0),
                FormulaValue::Number(4.0),
                FormulaValue::Number(5.0),
                FormulaValue::Number(6.0),
            ],
            2,
            3,
        );
        assert_eq!(
            eval_func("ROWS", std::slice::from_ref(&matrix)),
            FormulaValue::Number(2.0)
        );
        assert_eq!(
            eval_func("COLUMNS", std::slice::from_ref(&matrix)),
            FormulaValue::Number(3.0)
        );
        assert_eq!(
            eval_func("TRANSPOSE", std::slice::from_ref(&matrix)),
            arr(
                &[
                    FormulaValue::Number(1.0),
                    FormulaValue::Number(4.0),
                    FormulaValue::Number(2.0),
                    FormulaValue::Number(5.0),
                    FormulaValue::Number(3.0),
                    FormulaValue::Number(6.0),
                ],
                3,
                2,
            )
        );
        assert!(matches!(
            eval_func("TRANSPOSE", &[arr(&[], 0, 0)]),
            FormulaValue::Error(FormulaError::Value)
        ));
    }

    #[test]
    fn test_bounded_statistical_compatibility_helpers() {
        let values = arr(
            &[
                FormulaValue::Number(10.0),
                FormulaValue::Number(30.0),
                FormulaValue::Number(20.0),
                FormulaValue::Number(20.0),
            ],
            4,
            1,
        );
        assert_eq!(
            eval_func("RANK.EQ", &[FormulaValue::Number(20.0), values.clone()]),
            FormulaValue::Number(2.0)
        );
        assert_eq!(
            eval_func(
                "RANK",
                &[
                    FormulaValue::Number(30.0),
                    values.clone(),
                    FormulaValue::Number(1.0)
                ],
            ),
            FormulaValue::Number(4.0)
        );
        assert_eq!(
            eval_func("RANK.EQ", &[FormulaValue::Number(5.0), values.clone()],),
            FormulaValue::Error(FormulaError::Na)
        );

        let quartiles = arr(
            &[
                FormulaValue::Number(1.0),
                FormulaValue::Number(2.0),
                FormulaValue::Number(3.0),
                FormulaValue::Number(4.0),
            ],
            4,
            1,
        );
        assert_eq!(
            eval_func(
                "QUARTILE.INC",
                &[quartiles.clone(), FormulaValue::Number(1.0)]
            ),
            FormulaValue::Number(1.75)
        );
        assert_eq!(
            eval_func("QUARTILE", &[quartiles, FormulaValue::Number(2.0)]),
            FormulaValue::Number(2.5)
        );
        assert!(matches!(
            eval_func("QUARTILE.INC", &[values.clone(), FormulaValue::Number(5.0)]),
            FormulaValue::Error(FormulaError::DivZero)
        ));

        let x = arr(
            &[
                FormulaValue::Number(1.0),
                FormulaValue::Number(2.0),
                FormulaValue::Number(3.0),
            ],
            3,
            1,
        );
        let y = arr(
            &[
                FormulaValue::Number(2.0),
                FormulaValue::Number(4.0),
                FormulaValue::Number(6.0),
            ],
            3,
            1,
        );
        assert_eq!(
            eval_func("CORREL", &[x.clone(), y.clone()]),
            FormulaValue::Number(1.0)
        );
        assert_eq!(eval_func("PEARSON", &[x, y]), FormulaValue::Number(1.0));
        assert_eq!(
            eval_func(
                "CORREL",
                &[
                    arr(
                        &[FormulaValue::Number(1.0), FormulaValue::Number(2.0)],
                        2,
                        1
                    ),
                    arr(&[FormulaValue::Number(1.0)], 1, 1),
                ],
            ),
            FormulaValue::Error(FormulaError::Value)
        );

        assert_eq!(
            eval_func(
                "SUMSQ",
                &[arr(
                    &[
                        FormulaValue::Number(1.0),
                        FormulaValue::Number(2.0),
                        FormulaValue::Number(3.0),
                    ],
                    1,
                    3,
                )],
            ),
            FormulaValue::Number(14.0)
        );
    }

    #[test]
    fn test_bounded_dynamic_array_functions() {
        let values = arr(
            &[
                FormulaValue::String("A".into()),
                FormulaValue::Number(10.0),
                FormulaValue::String("B".into()),
                FormulaValue::Number(20.0),
                FormulaValue::String("A".into()),
                FormulaValue::Number(30.0),
            ],
            3,
            2,
        );
        let include = arr(
            &[
                FormulaValue::Boolean(true),
                FormulaValue::Boolean(false),
                FormulaValue::Boolean(true),
            ],
            3,
            1,
        );
        assert_eq!(
            eval_func("FILTER", &[values.clone(), include]),
            arr(
                &[
                    FormulaValue::String("A".into()),
                    FormulaValue::Number(10.0),
                    FormulaValue::String("A".into()),
                    FormulaValue::Number(30.0),
                ],
                2,
                2,
            )
        );

        assert_eq!(
            eval_func("UNIQUE", std::slice::from_ref(&values)),
            arr(
                &[
                    FormulaValue::String("A".into()),
                    FormulaValue::Number(10.0),
                    FormulaValue::String("B".into()),
                    FormulaValue::Number(20.0),
                    FormulaValue::String("A".into()),
                    FormulaValue::Number(30.0),
                ],
                3,
                2,
            )
        );
        let duplicates = arr(
            &[
                FormulaValue::String("A".into()),
                FormulaValue::String("A".into()),
                FormulaValue::String("B".into()),
            ],
            3,
            1,
        );
        assert_eq!(
            eval_func("UNIQUE", std::slice::from_ref(&duplicates)),
            arr(
                &[
                    FormulaValue::String("A".into()),
                    FormulaValue::String("B".into()),
                ],
                2,
                1,
            )
        );
        assert_eq!(
            eval_func(
                "UNIQUE",
                &[
                    duplicates,
                    FormulaValue::Boolean(false),
                    FormulaValue::Boolean(true)
                ],
            ),
            arr(&[FormulaValue::String("B".into())], 1, 1)
        );

        let sort_values = arr(
            &[
                FormulaValue::String("B".into()),
                FormulaValue::Number(20.0),
                FormulaValue::String("A".into()),
                FormulaValue::Number(10.0),
                FormulaValue::String("C".into()),
                FormulaValue::Number(30.0),
            ],
            3,
            2,
        );
        assert_eq!(
            eval_func("SORT", &[sort_values, FormulaValue::Number(2.0)]),
            arr(
                &[
                    FormulaValue::String("A".into()),
                    FormulaValue::Number(10.0),
                    FormulaValue::String("B".into()),
                    FormulaValue::Number(20.0),
                    FormulaValue::String("C".into()),
                    FormulaValue::Number(30.0),
                ],
                3,
                2,
            )
        );
        assert_eq!(
            eval_func(
                "SORT",
                &[
                    arr(
                        &[
                            FormulaValue::Number(3.0),
                            FormulaValue::Number(1.0),
                            FormulaValue::Number(2.0),
                            FormulaValue::String("c".into()),
                            FormulaValue::String("a".into()),
                            FormulaValue::String("b".into()),
                        ],
                        2,
                        3,
                    ),
                    FormulaValue::Number(1.0),
                    FormulaValue::Number(1.0),
                    FormulaValue::Boolean(true),
                ],
            ),
            arr(
                &[
                    FormulaValue::Number(1.0),
                    FormulaValue::Number(2.0),
                    FormulaValue::Number(3.0),
                    FormulaValue::String("a".into()),
                    FormulaValue::String("b".into()),
                    FormulaValue::String("c".into()),
                ],
                2,
                3,
            )
        );
        let sort_by_keys = arr(
            &[
                FormulaValue::String("A".into()),
                FormulaValue::Number(30.0),
                FormulaValue::String("B".into()),
                FormulaValue::Number(10.0),
                FormulaValue::String("C".into()),
                FormulaValue::Number(20.0),
            ],
            3,
            2,
        );
        let sort_by_order = arr(
            &[
                FormulaValue::Number(30.0),
                FormulaValue::Number(10.0),
                FormulaValue::Number(20.0),
            ],
            3,
            1,
        );
        assert_eq!(
            eval_func(
                "SORTBY",
                &[sort_by_keys, sort_by_order, FormulaValue::Number(1.0),],
            ),
            arr(
                &[
                    FormulaValue::String("B".into()),
                    FormulaValue::Number(10.0),
                    FormulaValue::String("C".into()),
                    FormulaValue::Number(20.0),
                    FormulaValue::String("A".into()),
                    FormulaValue::Number(30.0),
                ],
                3,
                2,
            )
        );
        let multi_key_values = arr(
            &[
                FormulaValue::String("x".into()),
                FormulaValue::Number(1.0),
                FormulaValue::String("a".into()),
                FormulaValue::String("y".into()),
                FormulaValue::Number(1.0),
                FormulaValue::String("b".into()),
                FormulaValue::String("z".into()),
                FormulaValue::Number(2.0),
                FormulaValue::String("c".into()),
            ],
            3,
            3,
        );
        assert_eq!(
            eval_func(
                "SORTBY",
                &[
                    multi_key_values,
                    arr(
                        &[
                            FormulaValue::Number(1.0),
                            FormulaValue::Number(1.0),
                            FormulaValue::Number(2.0),
                        ],
                        3,
                        1,
                    ),
                    FormulaValue::Number(1.0),
                    arr(
                        &[
                            FormulaValue::String("a".into()),
                            FormulaValue::String("b".into()),
                            FormulaValue::String("c".into()),
                        ],
                        3,
                        1,
                    ),
                    FormulaValue::Number(-1.0),
                ],
            ),
            arr(
                &[
                    FormulaValue::String("y".into()),
                    FormulaValue::Number(1.0),
                    FormulaValue::String("b".into()),
                    FormulaValue::String("x".into()),
                    FormulaValue::Number(1.0),
                    FormulaValue::String("a".into()),
                    FormulaValue::String("z".into()),
                    FormulaValue::Number(2.0),
                    FormulaValue::String("c".into()),
                ],
                3,
                3,
            )
        );
        assert_eq!(
            eval_func(
                "SORTBY",
                &[
                    arr(
                        &[
                            FormulaValue::Number(3.0),
                            FormulaValue::Number(1.0),
                            FormulaValue::Number(2.0),
                            FormulaValue::String("c".into()),
                            FormulaValue::String("a".into()),
                            FormulaValue::String("b".into()),
                        ],
                        2,
                        3,
                    ),
                    arr(
                        &[
                            FormulaValue::Number(3.0),
                            FormulaValue::Number(1.0),
                            FormulaValue::Number(2.0),
                        ],
                        1,
                        3,
                    ),
                ],
            ),
            arr(
                &[
                    FormulaValue::Number(1.0),
                    FormulaValue::Number(2.0),
                    FormulaValue::Number(3.0),
                    FormulaValue::String("a".into()),
                    FormulaValue::String("b".into()),
                    FormulaValue::String("c".into()),
                ],
                2,
                3,
            )
        );
        assert!(matches!(
            eval_func(
                "SORTBY",
                &[
                    arr(
                        &[FormulaValue::Number(1.0), FormulaValue::Number(2.0),],
                        2,
                        1,
                    ),
                    arr(
                        &[FormulaValue::Number(1.0), FormulaValue::Number(2.0)],
                        1,
                        2,
                    ),
                ],
            ),
            FormulaValue::Error(FormulaError::Value)
        ));
        let flatten = arr(
            &[
                FormulaValue::Number(1.0),
                FormulaValue::Empty,
                FormulaValue::Error(FormulaError::Value),
                FormulaValue::Number(2.0),
            ],
            2,
            2,
        );
        assert_eq!(
            eval_func("TOCOL", std::slice::from_ref(&flatten)),
            arr(
                &[
                    FormulaValue::Number(1.0),
                    FormulaValue::Empty,
                    FormulaValue::Error(FormulaError::Value),
                    FormulaValue::Number(2.0),
                ],
                4,
                1,
            )
        );
        assert_eq!(
            eval_func("TOCOL", &[flatten.clone(), FormulaValue::Number(3.0)],),
            arr(
                &[FormulaValue::Number(1.0), FormulaValue::Number(2.0)],
                2,
                1,
            )
        );
        assert_eq!(
            eval_func(
                "TOCOL",
                &[
                    arr(
                        &[
                            FormulaValue::String(String::new()),
                            FormulaValue::Number(7.0)
                        ],
                        1,
                        2,
                    ),
                    FormulaValue::Number(1.0),
                ],
            ),
            arr(&[FormulaValue::Number(7.0)], 1, 1)
        );
        assert_eq!(
            eval_func(
                "TOROW",
                &[
                    flatten,
                    FormulaValue::Number(0.0),
                    FormulaValue::Boolean(true)
                ],
            ),
            arr(
                &[
                    FormulaValue::Number(1.0),
                    FormulaValue::Error(FormulaError::Value),
                    FormulaValue::Empty,
                    FormulaValue::Number(2.0),
                ],
                1,
                4,
            )
        );
        let vector = arr(
            &[
                FormulaValue::Number(1.0),
                FormulaValue::Number(2.0),
                FormulaValue::Number(3.0),
                FormulaValue::Number(4.0),
                FormulaValue::Number(5.0),
            ],
            1,
            5,
        );
        assert_eq!(
            eval_func("WRAPROWS", &[vector.clone(), FormulaValue::Number(2.0)]),
            arr(
                &[
                    FormulaValue::Number(1.0),
                    FormulaValue::Number(2.0),
                    FormulaValue::Number(3.0),
                    FormulaValue::Number(4.0),
                    FormulaValue::Number(5.0),
                    FormulaValue::Error(FormulaError::Na),
                ],
                3,
                2,
            )
        );
        assert_eq!(
            eval_func(
                "WRAPCOLS",
                &[vector, FormulaValue::Number(2.0), FormulaValue::Number(0.0)],
            ),
            arr(
                &[
                    FormulaValue::Number(1.0),
                    FormulaValue::Number(3.0),
                    FormulaValue::Number(5.0),
                    FormulaValue::Number(2.0),
                    FormulaValue::Number(4.0),
                    FormulaValue::Number(0.0),
                ],
                2,
                3,
            )
        );
        assert!(matches!(
            eval_func(
                "WRAPROWS",
                &[
                    arr(&[FormulaValue::Number(1.0)], 1, 1),
                    FormulaValue::Number(0.0)
                ],
            ),
            FormulaValue::Error(FormulaError::Value)
        ));
        assert_eq!(
            eval_func("SEQUENCE", &[FormulaValue::Number(3.0)]),
            arr(
                &[
                    FormulaValue::Number(1.0),
                    FormulaValue::Number(2.0),
                    FormulaValue::Number(3.0),
                ],
                3,
                1,
            )
        );
        assert_eq!(
            eval_func(
                "SEQUENCE",
                &[
                    FormulaValue::Number(2.0),
                    FormulaValue::Number(3.0),
                    FormulaValue::Number(10.0),
                    FormulaValue::Number(-2.0),
                ],
            ),
            arr(
                &[
                    FormulaValue::Number(10.0),
                    FormulaValue::Number(8.0),
                    FormulaValue::Number(6.0),
                    FormulaValue::Number(4.0),
                    FormulaValue::Number(2.0),
                    FormulaValue::Number(0.0),
                ],
                2,
                3,
            )
        );
        assert_eq!(
            eval_func(
                "SEQUENCE",
                &[
                    FormulaValue::Number(3.0),
                    FormulaValue::Empty,
                    FormulaValue::Number(10.0),
                ],
            ),
            arr(
                &[
                    FormulaValue::Number(10.0),
                    FormulaValue::Number(11.0),
                    FormulaValue::Number(12.0),
                ],
                3,
                1,
            )
        );
        assert!(matches!(
            eval_func(
                "SEQUENCE",
                &[FormulaValue::Number(100_001.0), FormulaValue::Number(1.0)],
            ),
            FormulaValue::Error(FormulaError::Value)
        ));
        assert!(matches!(
            eval_func("SEQUENCE", &[FormulaValue::Number(2.5)]),
            FormulaValue::Error(FormulaError::Value)
        ));
        let random = eval_func(
            "RANDARRAY",
            &[
                FormulaValue::Number(2.0),
                FormulaValue::Number(3.0),
                FormulaValue::Number(10.0),
                FormulaValue::Number(20.0),
                FormulaValue::Boolean(true),
            ],
        );
        match random {
            FormulaValue::Array(values, rows, columns) => {
                assert_eq!((rows, columns), (2, 3));
                assert!(values.iter().all(|value| match value {
                    FormulaValue::Number(value) => {
                        (10.0..=20.0).contains(value) && value.fract() == 0.0
                    }
                    _ => false,
                }));
            }
            other => panic!("expected RANDARRAY output, got {other:?}"),
        }
        assert!(matches!(
            eval_func(
                "RANDARRAY",
                &[
                    FormulaValue::Number(2.0),
                    FormulaValue::Number(2.0),
                    FormulaValue::Number(5.0),
                    FormulaValue::Number(4.0)
                ],
            ),
            FormulaValue::Error(FormulaError::Value)
        ));
        let left = arr(
            &[FormulaValue::Number(1.0), FormulaValue::Number(2.0)],
            2,
            1,
        );
        let right = arr(
            &[
                FormulaValue::Number(3.0),
                FormulaValue::Number(4.0),
                FormulaValue::Number(5.0),
            ],
            3,
            1,
        );
        assert_eq!(
            eval_func("HSTACK", &[left.clone(), right.clone()]),
            arr(
                &[
                    FormulaValue::Number(1.0),
                    FormulaValue::Number(3.0),
                    FormulaValue::Number(2.0),
                    FormulaValue::Number(4.0),
                    FormulaValue::Empty,
                    FormulaValue::Number(5.0),
                ],
                3,
                2,
            )
        );
        assert_eq!(
            eval_func("VSTACK", &[left, right]),
            arr(
                &[
                    FormulaValue::Number(1.0),
                    FormulaValue::Number(2.0),
                    FormulaValue::Number(3.0),
                    FormulaValue::Number(4.0),
                    FormulaValue::Number(5.0),
                ],
                5,
                1,
            )
        );
        assert_eq!(
            eval_func("TAKE", &[values.clone(), FormulaValue::Number(2.0)]),
            arr(
                &[
                    FormulaValue::String("A".into()),
                    FormulaValue::Number(10.0),
                    FormulaValue::String("B".into()),
                    FormulaValue::Number(20.0),
                ],
                2,
                2,
            )
        );
        assert_eq!(
            eval_func("TAKE", &[values.clone(), FormulaValue::Number(-1.0)]),
            arr(
                &[FormulaValue::String("A".into()), FormulaValue::Number(30.0)],
                1,
                2,
            )
        );
        assert_eq!(
            eval_func(
                "DROP",
                &[
                    values.clone(),
                    FormulaValue::Number(1.0),
                    FormulaValue::Number(1.0),
                ],
            ),
            arr(
                &[FormulaValue::Number(20.0), FormulaValue::Number(30.0)],
                2,
                1,
            )
        );
        assert_eq!(
            eval_func("DROP", &[values.clone(), FormulaValue::Number(1.0)]),
            arr(
                &[
                    FormulaValue::String("B".into()),
                    FormulaValue::Number(20.0),
                    FormulaValue::String("A".into()),
                    FormulaValue::Number(30.0),
                ],
                2,
                2,
            )
        );
        assert_eq!(
            eval_func(
                "CHOOSECOLS",
                &[
                    values.clone(),
                    FormulaValue::Number(2.0),
                    FormulaValue::Number(1.0)
                ],
            ),
            arr(
                &[
                    FormulaValue::Number(10.0),
                    FormulaValue::String("A".into()),
                    FormulaValue::Number(20.0),
                    FormulaValue::String("B".into()),
                    FormulaValue::Number(30.0),
                    FormulaValue::String("A".into()),
                ],
                3,
                2,
            )
        );
        assert_eq!(
            eval_func(
                "CHOOSEROWS",
                &[
                    values.clone(),
                    FormulaValue::Number(3.0),
                    FormulaValue::Number(1.0)
                ],
            ),
            arr(
                &[
                    FormulaValue::String("A".into()),
                    FormulaValue::Number(30.0),
                    FormulaValue::String("A".into()),
                    FormulaValue::Number(10.0),
                ],
                2,
                2,
            )
        );
        assert!(matches!(
            eval_func("TAKE", &[values, FormulaValue::Number(0.0)]),
            FormulaValue::Error(FormulaError::Value)
        ));
        assert!(matches!(
            eval_func(
                "FILTER",
                &[arr(&[FormulaValue::Number(1.0)], 1, 1), arr(&[], 0, 0)]
            ),
            FormulaValue::Error(FormulaError::Value)
        ));
        assert_eq!(
            eval_func(
                "FILTER",
                &[
                    arr(&[FormulaValue::Number(1.0)], 1, 1),
                    arr(&[FormulaValue::Boolean(false)], 1, 1),
                    FormulaValue::String("none".into()),
                ]
            ),
            FormulaValue::String("none".into())
        );
    }

    #[test]
    fn test_match() {
        let lookup_array = arr(
            &[
                FormulaValue::Number(10.0),
                FormulaValue::Number(20.0),
                FormulaValue::Number(30.0),
            ],
            3,
            1,
        );
        assert_eq!(
            eval_func(
                "MATCH",
                &[
                    FormulaValue::Number(20.0),
                    lookup_array.clone(),
                    FormulaValue::Number(0.0),
                ]
            ),
            FormulaValue::Number(2.0)
        );
        assert_eq!(
            eval_func(
                "MATCH",
                &[
                    FormulaValue::Number(25.0),
                    lookup_array,
                    FormulaValue::Number(1.0),
                ]
            ),
            FormulaValue::Number(2.0)
        );
    }

    #[test]
    fn test_sumifs_countifs_averageif() {
        let dept = arr(
            &[
                FormulaValue::String("A".into()),
                FormulaValue::String("B".into()),
                FormulaValue::String("A".into()),
            ],
            3,
            1,
        );
        let scores = arr(
            &[
                FormulaValue::Number(10.0),
                FormulaValue::Number(20.0),
                FormulaValue::Number(30.0),
            ],
            3,
            1,
        );
        assert_eq!(
            eval_func(
                "SUMIFS",
                &[
                    scores.clone(),
                    dept.clone(),
                    FormulaValue::String("A".into()),
                ]
            ),
            FormulaValue::Number(40.0)
        );
        assert_eq!(
            eval_func(
                "COUNTIFS",
                &[dept.clone(), FormulaValue::String("A".into())]
            ),
            FormulaValue::Number(2.0)
        );
        assert_eq!(
            eval_func(
                "AVERAGEIF",
                &[dept, FormulaValue::String("A".into()), scores]
            ),
            FormulaValue::Number(20.0)
        );
    }

    #[test]
    fn test_sumproduct_and_max_min_ifs() {
        let quantities = arr(
            &[
                FormulaValue::Number(2.0),
                FormulaValue::Number(3.0),
                FormulaValue::Number(4.0),
            ],
            3,
            1,
        );
        let prices = arr(
            &[
                FormulaValue::Number(10.0),
                FormulaValue::Number(5.0),
                FormulaValue::Number(2.5),
            ],
            3,
            1,
        );
        assert_eq!(
            eval_func("SUMPRODUCT", &[quantities.clone(), prices]),
            FormulaValue::Number(45.0)
        );

        let departments = arr(
            &[
                FormulaValue::String("A".into()),
                FormulaValue::String("B".into()),
                FormulaValue::String("A".into()),
            ],
            3,
            1,
        );
        assert_eq!(
            eval_func(
                "MAXIFS",
                &[
                    quantities.clone(),
                    departments.clone(),
                    FormulaValue::String("A".into()),
                ],
            ),
            FormulaValue::Number(4.0)
        );
        assert_eq!(
            eval_func(
                "MINIFS",
                &[quantities, departments, FormulaValue::String("C".into()),],
            ),
            FormulaValue::Number(0.0)
        );
    }

    #[test]
    fn aggregate_functions_reject_shape_mismatches_and_errors() {
        let one = arr(
            &[FormulaValue::Number(1.0), FormulaValue::Number(2.0)],
            2,
            1,
        );
        let two = arr(
            &[FormulaValue::Number(3.0), FormulaValue::Number(4.0)],
            1,
            2,
        );
        assert_eq!(
            eval_func("SUMPRODUCT", &[one.clone(), two]),
            FormulaValue::Error(FormulaError::Value)
        );
        assert_eq!(
            eval_func(
                "MAXIFS",
                &[
                    one,
                    arr(&[FormulaValue::String("A".into())], 1, 1),
                    FormulaValue::String("A".into()),
                ],
            ),
            FormulaValue::Error(FormulaError::Value)
        );
        assert_eq!(
            eval_func(
                "SUMPRODUCT",
                &[
                    arr(&[FormulaValue::Error(FormulaError::Na)], 1, 1,),
                    FormulaValue::Number(1.0),
                ],
            ),
            FormulaValue::Error(FormulaError::Na)
        );
    }

    #[test]
    fn test_date_functions() {
        let serial = eval_func(
            "DATE",
            &[
                FormulaValue::Number(2020.0),
                FormulaValue::Number(1.0),
                FormulaValue::Number(15.0),
            ],
        );
        assert_eq!(serial, FormulaValue::Number(43845.0));
        assert_eq!(
            eval_func("YEAR", std::slice::from_ref(&serial)),
            FormulaValue::Number(2020.0)
        );
        assert_eq!(
            eval_func("MONTH", std::slice::from_ref(&serial)),
            FormulaValue::Number(1.0)
        );
        assert_eq!(eval_func("DAY", &[serial]), FormulaValue::Number(15.0));
        assert_eq!(
            eval_func("DATEVALUE", &[FormulaValue::String("2020-01-15".into())]),
            FormulaValue::Number(43845.0)
        );
    }

    #[test]
    fn test_time_and_weekday_functions() {
        let time = eval_func(
            "TIME",
            &[
                FormulaValue::Number(14.0),
                FormulaValue::Number(30.0),
                FormulaValue::Number(0.0),
            ],
        );
        assert!((as_number_arg(&time).unwrap() - 0.6041666666666666).abs() < 1e-12);
        assert_eq!(
            eval_func("HOUR", std::slice::from_ref(&time)),
            FormulaValue::Number(14.0)
        );
        assert_eq!(
            eval_func("MINUTE", std::slice::from_ref(&time)),
            FormulaValue::Number(30.0)
        );
        assert_eq!(
            eval_func("SECOND", std::slice::from_ref(&time)),
            FormulaValue::Number(0.0)
        );
        let date = eval_func(
            "DATE",
            &[
                FormulaValue::Number(2020.0),
                FormulaValue::Number(1.0),
                FormulaValue::Number(15.0),
            ],
        );
        assert_eq!(
            eval_func("WEEKDAY", std::slice::from_ref(&date)),
            FormulaValue::Number(4.0)
        );
        assert_eq!(
            eval_func("WEEKDAY", &[date, FormulaValue::Number(2.0)],),
            FormulaValue::Number(3.0)
        );
    }

    #[test]
    fn test_networkdays_with_holiday_and_bounds() {
        let start = eval_func(
            "DATE",
            &[
                FormulaValue::Number(2020.0),
                FormulaValue::Number(1.0),
                FormulaValue::Number(6.0),
            ],
        );
        let end = eval_func(
            "DATE",
            &[
                FormulaValue::Number(2020.0),
                FormulaValue::Number(1.0),
                FormulaValue::Number(12.0),
            ],
        );
        let holiday = eval_func(
            "DATE",
            &[
                FormulaValue::Number(2020.0),
                FormulaValue::Number(1.0),
                FormulaValue::Number(8.0),
            ],
        );
        assert_eq!(
            eval_func("NETWORKDAYS", &[start.clone(), end.clone()]),
            FormulaValue::Number(5.0)
        );
        assert_eq!(
            eval_func("NETWORKDAYS", &[start, end, arr(&[holiday], 1, 1),],),
            FormulaValue::Number(4.0)
        );
        assert_eq!(
            eval_func(
                "NETWORKDAYS",
                &[FormulaValue::Number(0.0), FormulaValue::Number(1_000_001.0),],
            ),
            FormulaValue::Error(FormulaError::Value)
        );
    }

    #[test]
    fn test_business_date_functions() {
        let start = eval_func(
            "DATE",
            &[
                FormulaValue::Number(2020.0),
                FormulaValue::Number(1.0),
                FormulaValue::Number(31.0),
            ],
        );
        assert_eq!(
            eval_func("EDATE", &[start.clone(), FormulaValue::Number(1.0)]),
            FormulaValue::Number(43890.0)
        );
        assert_eq!(
            eval_func("DAYS", &[FormulaValue::Number(43890.0), start.clone()]),
            FormulaValue::Number(29.0)
        );
        assert_eq!(
            eval_func("WORKDAY", &[start.clone(), FormulaValue::Number(1.0)]),
            FormulaValue::Number(43864.0)
        );
        let holiday = eval_func(
            "DATE",
            &[
                FormulaValue::Number(2020.0),
                FormulaValue::Number(2.0),
                FormulaValue::Number(3.0),
            ],
        );
        assert_eq!(
            eval_func(
                "WORKDAY",
                &[start, FormulaValue::Number(1.0), arr(&[holiday], 1, 1),],
            ),
            FormulaValue::Number(43865.0)
        );
        assert_eq!(
            eval_func(
                "WORKDAY",
                &[FormulaValue::Number(0.0), FormulaValue::Number(1_000_001.0)],
            ),
            FormulaValue::Error(FormulaError::Value)
        );
        assert_eq!(
            eval_func(
                "EDATE",
                &[
                    FormulaValue::Number(10_000_001.0),
                    FormulaValue::Number(1.0)
                ],
            ),
            FormulaValue::Error(FormulaError::Value)
        );
    }

    #[test]
    fn test_eomonth() {
        let jan = eval_func(
            "DATE",
            &[
                FormulaValue::Number(2020.0),
                FormulaValue::Number(1.0),
                FormulaValue::Number(15.0),
            ],
        );
        let eom = eval_func("EOMONTH", &[jan, FormulaValue::Number(0.0)]);
        assert_eq!(eval_func("DAY", &[eom]), FormulaValue::Number(31.0));
    }

    #[test]
    fn test_is_functions() {
        assert_eq!(
            eval_func("ISBLANK", &[FormulaValue::Empty]),
            FormulaValue::Boolean(true)
        );
        assert_eq!(
            eval_func("ISERROR", &[FormulaValue::Error(FormulaError::Na)]),
            FormulaValue::Boolean(true)
        );
        assert_eq!(
            eval_func("ISNUMBER", &[FormulaValue::Number(1.0)]),
            FormulaValue::Boolean(true)
        );
        assert_eq!(
            eval_func("ISTEXT", &[FormulaValue::String("x".into())]),
            FormulaValue::Boolean(true)
        );
        assert_eq!(
            eval_func("ISLOGICAL", &[FormulaValue::Boolean(true)]),
            FormulaValue::Boolean(true)
        );
    }

    #[test]
    fn test_value_and_text_funcs() {
        assert_eq!(
            eval_func("VALUE", &[FormulaValue::String("12.5".into())]),
            FormulaValue::Number(12.5)
        );
        assert_eq!(
            eval_func(
                "SUBSTITUTE",
                &[
                    FormulaValue::String("foo-bar".into()),
                    FormulaValue::String("-".into()),
                    FormulaValue::String("_".into()),
                ]
            ),
            FormulaValue::String("foo_bar".into())
        );
        assert_eq!(
            eval_func(
                "REPLACE",
                &[
                    FormulaValue::String("abcdef".into()),
                    FormulaValue::Number(2.0),
                    FormulaValue::Number(3.0),
                    FormulaValue::String("X".into()),
                ]
            ),
            FormulaValue::String("aXef".into())
        );
        assert_eq!(
            eval_func(
                "FIND",
                &[
                    FormulaValue::String("b".into()),
                    FormulaValue::String("abc".into()),
                ]
            ),
            FormulaValue::Number(2.0)
        );
        assert_eq!(
            eval_func(
                "SEARCH",
                &[
                    FormulaValue::String("B".into()),
                    FormulaValue::String("abc".into()),
                ]
            ),
            FormulaValue::Number(2.0)
        );
    }

    #[test]
    fn test_randbetween_range() {
        let val = eval_func(
            "RANDBETWEEN",
            &[FormulaValue::Number(1.0), FormulaValue::Number(3.0)],
        );
        if let FormulaValue::Number(n) = val {
            assert!((1.0..=3.0).contains(&n));
        } else {
            panic!("expected number");
        }
    }

    #[test]
    fn test_hlookup() {
        let table = arr(
            &[
                FormulaValue::Number(10.0),
                FormulaValue::Number(20.0),
                FormulaValue::Number(100.0),
                FormulaValue::Number(200.0),
            ],
            2,
            2,
        );
        assert_eq!(
            eval_func(
                "HLOOKUP",
                &[
                    FormulaValue::Number(20.0),
                    table,
                    FormulaValue::Number(2.0),
                    FormulaValue::Boolean(false),
                ]
            ),
            FormulaValue::Number(200.0)
        );
    }

    #[test]
    fn test_choose() {
        assert_eq!(
            eval_func(
                "CHOOSE",
                &[
                    FormulaValue::Number(2.0),
                    FormulaValue::String("a".into()),
                    FormulaValue::String("b".into()),
                    FormulaValue::String("c".into()),
                ]
            ),
            FormulaValue::String("b".into())
        );
    }

    #[test]
    fn test_financial_functions() {
        let pmt = match eval_func(
            "PMT",
            &[
                FormulaValue::Number(0.1),
                FormulaValue::Number(3.0),
                FormulaValue::Number(1000.0),
            ],
        ) {
            FormulaValue::Number(value) => value,
            other => panic!("expected PMT number, got {other:?}"),
        };
        assert!((pmt + 402.114803625).abs() < 1e-8);

        let pv = match eval_func(
            "PV",
            &[
                FormulaValue::Number(0.1),
                FormulaValue::Number(3.0),
                FormulaValue::Number(pmt),
            ],
        ) {
            FormulaValue::Number(value) => value,
            other => panic!("expected PV number, got {other:?}"),
        };
        assert!((pv - 1000.0).abs() < 1e-8);

        let fv = match eval_func(
            "FV",
            &[
                FormulaValue::Number(0.1),
                FormulaValue::Number(3.0),
                FormulaValue::Number(pmt),
                FormulaValue::Number(1000.0),
            ],
        ) {
            FormulaValue::Number(value) => value,
            other => panic!("expected FV number, got {other:?}"),
        };
        assert!(fv.abs() < 1e-8);

        let cashflows = arr(
            &[
                FormulaValue::Number(-100.0),
                FormulaValue::Number(60.0),
                FormulaValue::Number(60.0),
            ],
            3,
            1,
        );
        let npv = eval_func("NPV", &[FormulaValue::Number(0.1), cashflows.clone()]);
        match npv {
            FormulaValue::Number(value) => assert!((value - 3.7565740045078755).abs() < 1e-12),
            other => panic!("expected NPV number, got {other:?}"),
        }
        let irr = match eval_func("IRR", &[cashflows]) {
            FormulaValue::Number(value) => value,
            other => panic!("expected IRR number, got {other:?}"),
        };
        assert!((irr - 0.1306623862918075).abs() < 1e-8);
    }

    #[test]
    fn financial_functions_reject_invalid_inputs() {
        assert_eq!(
            eval_func(
                "PMT",
                &[
                    FormulaValue::Number(0.1),
                    FormulaValue::Number(0.0),
                    FormulaValue::Number(100.0),
                ],
            ),
            FormulaValue::Error(FormulaError::DivZero)
        );
        assert_eq!(
            eval_func(
                "PV",
                &[
                    FormulaValue::Number(0.1),
                    FormulaValue::Number(3.0),
                    FormulaValue::Number(10.0),
                    FormulaValue::Number(0.0),
                    FormulaValue::Number(2.0),
                ],
            ),
            FormulaValue::Error(FormulaError::Value)
        );
        assert_eq!(
            eval_func("IRR", &[arr(&[FormulaValue::Number(1.0)], 1, 1)]),
            FormulaValue::Error(FormulaError::Num)
        );
    }

    #[test]
    fn numeric_functions_reject_non_finite_results() {
        assert_eq!(
            eval_func(
                "POWER",
                &[FormulaValue::Number(f64::MAX), FormulaValue::Number(2.0),],
            ),
            FormulaValue::Error(FormulaError::Num)
        );
        assert_eq!(
            eval_func(
                "SUM",
                &[
                    FormulaValue::Number(f64::MAX),
                    FormulaValue::Number(f64::MAX)
                ]
            ),
            FormulaValue::Error(FormulaError::Num)
        );
        assert_eq!(
            eval_func("ABS", &[FormulaValue::Number(f64::NAN)]),
            FormulaValue::Error(FormulaError::Num)
        );
    }
}
