use crate::ast::{FormulaError, FormulaValue};

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
        "TRUE" => FormulaValue::Boolean(true),
        "FALSE" => FormulaValue::Boolean(false),
        _ => FormulaValue::Error(FormulaError::Name),
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
}
