use crate::cell::SheetCell;
use crate::workbook::SheetData;

pub fn export_sheet_to_csv(sheet: &SheetData) -> String {
    let mut max_row = 0;
    let mut max_col = 0;
    for key in sheet.cells.keys() {
        if let Ok((r, c)) = crate::workbook::parse_key(key) {
            if r > max_row {
                max_row = r;
            }
            if c > max_col {
                max_col = c;
            }
        }
    }

    let mut lines = Vec::new();
    for r in 1..=max_row {
        let mut row_vals = Vec::new();
        for c in 1..=max_col {
            let key = format!("{}:{}", r, c);
            let val = sheet
                .cells
                .get(&key)
                .map(|cell| cell.raw_value.clone())
                .unwrap_or_default();

            // Handle CSV quoting
            if val.contains(',') || val.contains('"') || val.contains('\n') {
                let escaped = val.replace('"', "\"\"");
                row_vals.push(format!("\"{}\"", escaped));
            } else {
                row_vals.push(val);
            }
        }
        lines.push(row_vals.join(","));
    }

    lines.join("\n")
}

pub fn import_csv_to_sheet(csv_content: &str, sheet_name: &str) -> SheetData {
    import_csv_to_sheet_with_delimiter(csv_content, sheet_name, detect_delimiter(csv_content))
}

pub fn import_csv_bytes_to_sheet(csv_bytes: &[u8], sheet_name: &str) -> SheetData {
    import_csv_bytes_to_sheet_with_options(csv_bytes, sheet_name, None, "auto")
}

pub fn import_csv_bytes_to_sheet_with_options(
    csv_bytes: &[u8],
    sheet_name: &str,
    delimiter: Option<char>,
    encoding: &str,
) -> SheetData {
    let content = match encoding.to_ascii_lowercase().as_str() {
        "latin-1" | "latin1" => csv_bytes.iter().map(|byte| char::from(*byte)).collect(),
        _ => match std::str::from_utf8(csv_bytes) {
            Ok(content) => content.to_string(),
            Err(_) => csv_bytes.iter().map(|byte| char::from(*byte)).collect(),
        },
    };
    let delimiter = delimiter.unwrap_or_else(|| detect_delimiter(&content));
    import_csv_to_sheet_with_delimiter(&content, sheet_name, delimiter)
}

fn detect_delimiter(csv_content: &str) -> char {
    let first_line = csv_content.lines().next().unwrap_or_default();
    [',', ';', '\t']
        .into_iter()
        .max_by_key(|delimiter| first_line.matches(*delimiter).count())
        .unwrap_or(',')
}

fn import_csv_to_sheet_with_delimiter(
    csv_content: &str,
    sheet_name: &str,
    delimiter: char,
) -> SheetData {
    let mut sheet = SheetData::new("imported-csv", sheet_name);
    let input = csv_content.trim_start_matches('\u{feff}');
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut row = Vec::new();
    let mut field = String::new();
    let mut in_quotes = false;
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        match ch {
            '"' if in_quotes && chars.peek() == Some(&'"') => {
                field.push('"');
                chars.next();
            }
            '"' => in_quotes = !in_quotes,
            value if value == delimiter && !in_quotes => {
                row.push(std::mem::take(&mut field));
            }
            '\n' if !in_quotes => {
                row.push(std::mem::take(&mut field));
                rows.push(std::mem::take(&mut row));
            }
            '\r' if !in_quotes => {}
            _ => field.push(ch),
        }
    }
    if !field.is_empty() || !row.is_empty() {
        row.push(field);
        rows.push(row);
    }

    for (r_idx, values) in rows.into_iter().enumerate() {
        for (c_idx, value) in values.into_iter().enumerate() {
            if value.is_empty() {
                continue;
            }
            let raw = value.trim_start_matches('\u{feff}').to_string();
            sheet.cells.insert(
                format!("{}:{}", r_idx + 1, c_idx + 1),
                SheetCell {
                    raw_value: raw.clone(),
                    display_value: raw,
                    formula: None,
                    style: None,
                },
            );
        }
    }
    sheet
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn imports_quoted_commas_quotes_and_newlines() {
        let sheet = import_csv_to_sheet(
            "\u{feff}Name,Note\n\"A, B\",\"say \"\"hi\"\"\"\nC,\"two\nlines\"",
            "Test",
        );
        assert_eq!(sheet.cells["1:1"].raw_value, "Name");
        assert_eq!(sheet.cells["2:1"].raw_value, "A, B");
        assert_eq!(sheet.cells["2:2"].raw_value, "say \"hi\"");
        assert_eq!(sheet.cells["3:2"].raw_value, "two\nlines");
    }

    #[test]
    fn detects_semicolon_and_latin1_csv() {
        let sheet = import_csv_bytes_to_sheet(b"Name;City\nAndre;Paris\n", "Test");
        assert_eq!(sheet.cells["2:1"].raw_value, "Andre");
        assert_eq!(sheet.cells["2:2"].raw_value, "Paris");

        let latin1 = [b'N', b'a', b'm', b'e', b',', 0xC9, b't', b'\n'];
        let sheet = import_csv_bytes_to_sheet(&latin1, "Latin1");
        assert_eq!(sheet.cells["1:2"].raw_value, "Ét");
    }
}
