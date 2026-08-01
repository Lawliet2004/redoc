use crate::ast::{FormulaError, FormulaValue};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::thread;
use std::time::SystemTime;

const EXCEL_EPOCH_YEAR: i32 = 1899;
const EXCEL_EPOCH_MONTH: i32 = 12;
const EXCEL_EPOCH_DAY: i32 = 30;

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

pub fn eval_func(name: &str, args: &[FormulaValue]) -> FormulaValue {
    match name {
        "SUM" => {
            let mut sum = 0.0;
            for arg in &flatten_args(args) {
                if let FormulaValue::Number(n) = arg {
                    sum += n;
                } else if let FormulaValue::Error(e) = arg {
                    return FormulaValue::Error(e.clone());
                }
            }
            FormulaValue::Number(sum)
        }
        "AVERAGE" => {
            let mut sum = 0.0;
            let mut count = 0;
            for arg in &flatten_args(args) {
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
                FormulaValue::Number(sum / count as f64)
            }
        }
        "MIN" => {
            let mut min_val = f64::INFINITY;
            let mut found = false;
            for arg in &flatten_args(args) {
                if let FormulaValue::Number(n) = arg {
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
            for arg in &flatten_args(args) {
                if let FormulaValue::Number(n) = arg {
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
            let args_flat = flatten_args(args);
            let count = args_flat
                .iter()
                .filter(|a| matches!(a, FormulaValue::Number(_)))
                .count();
            FormulaValue::Number(count as f64)
        }
        "COUNTA" => {
            let args_flat = flatten_args(args);
            let count = args_flat
                .iter()
                .filter(|a| !matches!(a, FormulaValue::Empty))
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
            for arg in &flatten_args(args) {
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
            for arg in &flatten_args(args) {
                match arg {
                    FormulaValue::Boolean(b) if *b => return FormulaValue::Boolean(true),
                    FormulaValue::Number(n) if *n != 0.0 => return FormulaValue::Boolean(true),
                    FormulaValue::Error(e) => return FormulaValue::Error(e.clone()),
                    _ => {}
                }
            }
            FormulaValue::Boolean(false)
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
                FormulaValue::Number((n * mult).round() / mult)
            }
        }
        "CONCAT" => {
            let mut res = String::new();
            for arg in &flatten_args(args) {
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
                FormulaValue::Number(n) => FormulaValue::Number(n.abs()),
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
            let range = flatten_args(&args[..1]);
            let count = range
                .iter()
                .filter(|a| matches_criterion(a, &criterion))
                .count();
            FormulaValue::Number(count as f64)
        }
        "SUMIF" => {
            if args.len() < 2 {
                return FormulaValue::Error(FormulaError::Value);
            }
            let range = flatten_args(&args[0..1]);
            let criterion = match &args[1] {
                FormulaValue::String(s) => s.clone(),
                FormulaValue::Number(n) => n.to_string(),
                FormulaValue::Boolean(b) => b.to_string(),
                FormulaValue::Error(e) => return FormulaValue::Error(e.clone()),
                FormulaValue::Empty => String::new(),
                FormulaValue::Array(..) => return FormulaValue::Error(FormulaError::Value),
            };
            let sum_range = if args.len() >= 3 {
                flatten_args(&args[2..3])
            } else {
                range.clone()
            };
            let mut sum = 0.0;
            for (i, val) in range.iter().enumerate() {
                if matches_criterion(val, &criterion) {
                    if let Some(FormulaValue::Number(n)) = sum_range.get(i) {
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
            FormulaValue::Number((n / sig).ceil() * sig)
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
            FormulaValue::Number((n / sig).floor() * sig)
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
            FormulaValue::Number(n - d * (n / d).floor())
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
            FormulaValue::Number(n.powf(p))
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
            for arg in &flatten_args(args) {
                if let FormulaValue::Number(n) = arg {
                    prod *= n;
                    found = true;
                } else if let FormulaValue::Error(e) = arg {
                    return FormulaValue::Error(e.clone());
                }
            }
            if found {
                FormulaValue::Number(prod)
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
        "SUMIFS" => eval_sumifs(args),
        "COUNTIFS" => eval_countifs(args),
        "AVERAGEIF" => eval_averageif(args),
        "AVERAGEIFS" => eval_averageifs(args),
        "DATE" => eval_date(args),
        "DATEVALUE" => eval_datevalue(args),
        "YEAR" => eval_year(args),
        "MONTH" => eval_month(args),
        "DAY" => eval_day(args),
        "EOMONTH" => eval_eomonth(args),
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
        "RANDBETWEEN" => eval_randbetween(args),
        "HLOOKUP" => eval_hlookup(args),
        "CHOOSE" => eval_choose(args),
        "TRUE" => FormulaValue::Boolean(true),
        "FALSE" => FormulaValue::Boolean(false),
        _ => FormulaValue::Error(FormulaError::Name),
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

    let (lookup_data, lookup_rows, lookup_cols) = array_dims(&args[1])
        .unwrap_or((vec![], 0, 0));
    let (return_data, return_rows, return_cols) = array_dims(&args[2])
        .unwrap_or((vec![], 0, 0));

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
        let lookup_idx = if vertical {
            i
        } else if horizontal {
            i
        } else {
            i
        };
        let lookup_val = lookup_data
            .get(lookup_idx)
            .cloned()
            .unwrap_or(FormulaValue::Empty);
        if values_equal(&lookup_val, &lookup) {
            let return_idx = if return_rows == 1 && return_cols > 1 {
                i
            } else if return_cols == 1 && return_rows > 1 {
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
        let idx = (row_num - 1) as usize;
        return data
            .get(idx)
            .cloned()
            .unwrap_or(FormulaValue::Error(FormulaError::Ref));
    }
    if cols == 1 && rows > 1 {
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
    if args.len() < 3 || args.len() % 2 == 0 {
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

fn eval_countifs(args: &[FormulaValue]) -> FormulaValue {
    if args.len() < 2 || args.len() % 2 != 0 {
        return FormulaValue::Error(FormulaError::Value);
    }
    let first_range = flatten_args(&args[0..1]);
    let pairs = &args[..];
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
    if args.len() < 3 || args.len() % 2 == 0 {
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

fn parse_date_text(text: &str) -> Option<(i32, i32, i32)> {
    let parts: Vec<&str> = text.split(&['/', '-', '.', ' '][..])
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
    } else if c > 31 {
        Some((c, a, b))
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
        (FormulaValue::Number(x), FormulaValue::String(y)) => y
            .parse::<f64>()
            .map(|n| x <= &n)
            .unwrap_or(false),
        (FormulaValue::String(x), FormulaValue::Number(y)) => x
            .parse::<f64>()
            .map(|n| &n <= y)
            .unwrap_or(false),
        _ => false,
    }
}

fn vlookup_greater_or_equal(a: &FormulaValue, b: &FormulaValue) -> bool {
    match (a, b) {
        (FormulaValue::Number(x), FormulaValue::Number(y)) => x >= y,
        (FormulaValue::String(x), FormulaValue::String(y)) => x >= y,
        (FormulaValue::Number(x), FormulaValue::String(y)) => y
            .parse::<f64>()
            .map(|n| x >= &n)
            .unwrap_or(false),
        (FormulaValue::String(x), FormulaValue::Number(y)) => x
            .parse::<f64>()
            .map(|n| &n >= y)
            .unwrap_or(false),
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
            &[
                FormulaValue::Number(20.0),
                lookup_range,
                return_range,
            ],
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
            eval_func("YEAR", &[serial.clone()]),
            FormulaValue::Number(2020.0)
        );
        assert_eq!(
            eval_func("MONTH", &[serial.clone()]),
            FormulaValue::Number(1.0)
        );
        assert_eq!(
            eval_func("DAY", &[serial]),
            FormulaValue::Number(15.0)
        );
        assert_eq!(
            eval_func("DATEVALUE", &[FormulaValue::String("2020-01-15".into())]),
            FormulaValue::Number(43845.0)
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
        assert_eq!(
            eval_func("DAY", &[eom]),
            FormulaValue::Number(31.0)
        );
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
            assert!(n >= 1.0 && n <= 3.0);
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
}
