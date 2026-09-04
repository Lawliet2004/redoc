//! Native XLSX pivotTable / pivotCache OOXML parts.
//!
//! Emits real `xl/pivotCache/pivotCacheDefinition{N}.xml`,
//! `xl/pivotCache/pivotCacheRecords{N}.xml`, `xl/pivotTables/pivotTable{N}.xml`
//! (+ their relationship parts and worksheet/workbook wiring) so Excel opens
//! the pivots natively. Caches set `refreshOnLoad="1"`, so Excel rebuilds the
//! rendered grid from the records on open — we emit minimal item lists and let
//! Excel refresh rather than computing a full cell matrix ourselves.
//!
//! Import parses the same subset back into `PivotTableModel`s.

use crate::pdf::ExportError;
use redoc_sheet_engine::{CellRange, PivotTableModel, SheetData, WorkbookModel};
use std::collections::HashMap;

const MAX_PIVOT_PARTS: usize = 64;
const MAX_PIVOT_FIELDS: usize = 256;
const MAX_PIVOT_RECORDS: usize = 100_000;
const MAX_SHARED_ITEMS: usize = 500;

pub(crate) struct PivotPlan {
    pub cache_id: u32,
    pub sheet_index: usize,
    pub pivot: PivotTableModel,
    pub sheet_name: String,
    pub fields: Vec<String>,
    /// Data rows of the source range; `records[i][j]` is column j of row i.
    pub records: Vec<Vec<String>>,
}

pub(crate) fn plan_pivot_parts(workbook: &WorkbookModel) -> Vec<PivotPlan> {
    let mut plans = Vec::new();
    let mut next_cache_id = 1u32;
    for (sheet_index, sheet) in workbook.sheets.iter().enumerate() {
        for pivot in &sheet.pivot_tables {
            if plans.len() >= MAX_PIVOT_PARTS {
                return plans;
            }
            if let Some(plan) = plan_single_pivot(sheet, pivot, next_cache_id, sheet_index) {
                next_cache_id = next_cache_id.saturating_add(1);
                plans.push(plan);
            }
        }
    }
    plans
}

fn plan_single_pivot(
    sheet: &SheetData,
    pivot: &PivotTableModel,
    cache_id: u32,
    sheet_index: usize,
) -> Option<PivotPlan> {
    let source = &pivot.source_range;
    if source.start_row == 0
        || source.end_row < source.start_row
        || source.end_col < source.start_col
        || source.end_row == source.start_row
    {
        return None;
    }
    let width = (source.end_col - source.start_col + 1) as usize;
    if width > MAX_PIVOT_FIELDS {
        return None;
    }
    let in_range = |field: u32| field >= source.start_col && field <= source.end_col;
    if !in_range(pivot.row_field)
        || !in_range(pivot.value_field)
        || pivot.column_field.is_some_and(|field| !in_range(field))
    {
        return None;
    }

    let mut fields = Vec::with_capacity(width);
    for column in source.start_col..=source.end_col {
        let fallback = column_letter(column);
        let label = sheet
            .cells
            .get(&format!("{}:{}", source.start_row, column))
            .map(|cell| cell.display_value.trim())
            .filter(|value| !value.is_empty())
            .unwrap_or(fallback.as_str());
        fields.push(label.to_string());
    }

    let mut records = Vec::new();
    for row in (source.start_row + 1)..=source.end_row {
        if records.len() >= MAX_PIVOT_RECORDS {
            break;
        }
        let mut record = Vec::with_capacity(width);
        for column in source.start_col..=source.end_col {
            record.push(
                sheet
                    .cells
                    .get(&format!("{}:{}", row, column))
                    .map(|cell| cell.raw_value.clone())
                    .unwrap_or_default(),
            );
        }
        records.push(record);
    }

    Some(PivotPlan {
        cache_id,
        sheet_index,
        pivot: pivot.clone(),
        sheet_name: sheet.name.clone(),
        fields,
        records,
    })
}

pub(crate) fn column_letter(mut column: u32) -> String {
    let mut letters = String::new();
    while column > 0 {
        let remainder = (column - 1) % 26;
        letters.insert(
            0,
            char::from_u32(u32::from(b'A') + remainder).unwrap_or('A'),
        );
        column = (column - 1) / 26;
    }
    letters
}

fn xml_attr(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&apos;"),
            '\r' => escaped.push_str("&#13;"),
            '\n' => escaped.push_str("&#10;"),
            '\t' => escaped.push_str("&#9;"),
            c if c == '\u{0}' || (c < '\u{20}' && c != '\u{9}' && c != '\u{A}' && c != '\u{D}') => {
            }
            c => escaped.push(c),
        }
    }
    escaped
}

/// Excel subtotal opcode: sum=0x2, count=0x40, average=0x400.
fn data_subtotal(aggregation: &str) -> u16 {
    match aggregation.trim().to_ascii_lowercase().as_str() {
        "count" => 0x40,
        "average" => 0x400,
        _ => 0x2,
    }
}

fn agg_label(aggregation: &str) -> &'static str {
    match aggregation.trim().to_ascii_lowercase().as_str() {
        "count" => "Count",
        "average" => "Average",
        _ => "Sum",
    }
}

fn quoted_sheet_name(sheet_name: &str) -> String {
    let escaped = sheet_name.replace('\'', "''");
    let needs_quotes = sheet_name
        .chars()
        .any(|c| !c.is_ascii_alphanumeric() && c != '_');
    if needs_quotes {
        format!("'{escaped}'")
    } else {
        escaped
    }
}

fn cache_definition_xml(plan: &PivotPlan) -> String {
    let source = &plan.pivot.source_range;
    let ref_range = format!(
        "{}!{}{}:{}{}",
        quoted_sheet_name(&plan.sheet_name),
        column_letter(source.start_col),
        source.start_row,
        column_letter(source.end_col),
        source.end_row
    );

    let mut fields_xml = String::new();
    for (index, label) in plan.fields.iter().enumerate() {
        let column = source.start_col + index as u32;
        let is_row = column == plan.pivot.row_field;
        let is_value = column == plan.pivot.value_field;
        let is_column = plan.pivot.column_field == Some(column);
        let axis = if is_row || is_column {
            format!(" axis=\"axis{}\"", if is_column { 2 } else { 1 })
        } else {
            String::new()
        };
        let data_field = if is_value { " dataField=\"1\"" } else { "" };
        fields_xml.push_str(&format!(
            "<cacheField name=\"{}\" numFmtId=\"0\"{}{}>",
            xml_attr(label),
            axis,
            data_field
        ));
        if is_row || is_column {
            let mut shared: Vec<&String> = Vec::new();
            for record in &plan.records {
                let value = record.get(index);
                if value.is_none_or(|value| value.is_empty()) || shared.len() >= MAX_SHARED_ITEMS {
                    continue;
                }
                if !shared.contains(&value.unwrap()) {
                    shared.push(value.unwrap());
                }
            }
            fields_xml.push_str(&format!("<sharedItems count=\"{}\">", shared.len()));
            for value in &shared {
                fields_xml.push_str(&format!("<s v=\"{}\"/>", xml_attr(value)));
            }
            fields_xml.push_str("</sharedItems>");
        } else {
            fields_xml.push_str("<sharedItems/>");
        }
        fields_xml.push_str("</cacheField>");
    }

    format!(
        concat!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n",
            "<pivotCacheDefinition xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\" ",
            "xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\" ",
            "r:id=\"rId1\" refreshOnLoad=\"1\" refreshedBy=\"Redoc\" refreshedDate=\"4500.0\" ",
            "createdVersion=\"3\" refreshedVersion=\"3\" minRefreshableVersion=\"3\" ",
            "recordCount=\"{record_count}\">",
            "<cacheSource type=\"worksheet\"><worksheetSource ref=\"{source_ref}\"/></cacheSource>",
            "<cacheFields count=\"{field_count}\">{fields}</cacheFields>",
            "</pivotCacheDefinition>"
        ),
        record_count = plan.records.len(),
        source_ref = xml_attr(&ref_range),
        field_count = plan.fields.len(),
        fields = fields_xml,
    )
}

fn cache_records_xml(plan: &PivotPlan) -> String {
    let mut xml = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
         <pivotCacheRecords xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\" \
         xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\" count=\"",
    );
    xml.push_str(&plan.records.len().to_string());
    xml.push_str("\">");
    for record in &plan.records {
        xml.push_str("<r>");
        for value in record {
            let trimmed = value.trim();
            if let Ok(number) = trimmed.parse::<f64>() {
                if number.is_finite() {
                    xml.push_str(&format!("<n v=\"{number}\"/>"));
                    continue;
                }
            }
            match trimmed.to_ascii_lowercase().as_str() {
                "true" | "false" => {
                    xml.push_str(&format!(
                        "<b v=\"{}\"/>",
                        trimmed.eq_ignore_ascii_case("true")
                    ));
                }
                _ => xml.push_str(&format!("<s v=\"{}\"/>", xml_attr(value))),
            }
        }
        xml.push_str("</r>");
    }
    xml.push_str("</pivotCacheRecords>");
    xml
}

fn pivot_table_xml(plan: &PivotPlan, table_no: usize) -> String {
    let pivot = &plan.pivot;
    let source = &pivot.source_range;
    let field_index = |column: u32| (column - source.start_col) as usize;

    // Distinct axis values in first-seen order for the row/column fields.
    let axis_values = |field: u32| -> Vec<String> {
        let mut seen: Vec<String> = Vec::new();
        for record in &plan.records {
            let value = record.get(field_index(field)).cloned().unwrap_or_default();
            if value.is_empty() || seen.len() >= MAX_SHARED_ITEMS {
                continue;
            }
            if !seen.contains(&value) {
                seen.push(value);
            }
        }
        seen
    };
    let row_values = axis_values(pivot.row_field);
    let column_values = pivot.column_field.map(axis_values).unwrap_or_default();

    // pivotFields in cacheField order (required by the spec).
    let mut pivot_fields = String::new();
    for (index, _) in plan.fields.iter().enumerate() {
        let column = source.start_col + index as u32;
        if column == pivot.row_field {
            pivot_fields.push_str(&items_field("axis1", &row_values));
        } else if pivot.column_field == Some(column) {
            pivot_fields.push_str(&items_field("axis2", &column_values));
        } else {
            pivot_fields.push_str("<pivotField showAll=\"0\"/>");
        }
    }

    let value_header = plan
        .fields
        .get(field_index(pivot.value_field))
        .cloned()
        .unwrap_or_else(|| "Values".to_string());

    let location_first_row = pivot.output_start_row;
    let location_first_col = pivot.output_start_col;
    let has_column_axis = pivot.column_field.is_some();
    let location_row_count = pivot
        .output_row_count
        .unwrap_or_else(|| (row_values.len().saturating_add(2)).max(2) as u32);
    let location_col_count = pivot.output_col_count.unwrap_or_else(|| {
        (column_values
            .len()
            .saturating_add(if has_column_axis { 3 } else { 2 }))
        .max(2) as u32
    });
    let location = format!(
        "{}{}:{}{}",
        column_letter(location_first_col),
        location_first_row,
        column_letter(location_first_col + location_col_count - 1),
        location_first_row + location_row_count - 1
    );
    // firstDataRow/firstDataCol are offsets from the location's top-left cell.
    let first_data_row = 1 + u32::from(has_column_axis);
    let first_data_col = 1 + u32::from(has_column_axis);

    let row_items = format!(
        "<rowItems count=\"{}\">{}<i t=\"grand\"><x/></i></rowItems>",
        row_values.len() + 1,
        row_values
            .iter()
            .enumerate()
            .map(|(index, _)| format!("<i><x v=\"{index}\"/></i>"))
            .collect::<String>()
    );
    let col_items = if has_column_axis {
        format!(
            "<colItems count=\"{}\">{}<i t=\"grand\"><x/></i></colItems>",
            column_values.len() + 1,
            column_values
                .iter()
                .enumerate()
                .map(|(index, _)| format!("<i><x v=\"{index}\"/></i>"))
                .collect::<String>()
        )
    } else {
        "<colItems count=\"1\"><i/></colItems>".to_string()
    };
    let column_field_xml = pivot
        .column_field
        .map(|field| format!("<field x=\"{}\"/>", field_index(field)))
        .unwrap_or_default();

    format!(
        concat!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n",
            "<pivotTableDefinition xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\" ",
            "name=\"PivotTable{table_no}\" cacheId=\"{cache_id}\" applyNumberFormats=\"0\" ",
            "applyBorderFormats=\"0\" applyFontFormats=\"0\" applyPatternFormats=\"0\" ",
            "applyAlignmentFormats=\"0\" applyWidthHeightFormats=\"1\" dataCaption=\"Values\" ",
            "updatedVersion=\"3\" minRefreshableVersion=\"3\" createdVersion=\"3\" ",
            "indent=\"0\" outline=\"1\" outlineData=\"1\" multipleFieldFilters=\"0\">",
            "<location ref=\"{location}\" firstHeaderRow=\"1\" firstDataRow=\"{first_data_row}\" firstDataCol=\"{first_data_col}\"/>",
            "<pivotFields count=\"{field_count}\">{pivot_fields}</pivotFields>",
            "<rowFields count=\"1\"><field x=\"{row_field}\"/></rowFields>",
            "{row_items}",
            "<colFields count=\"{col_field_count}\">{column_fields}</colFields>",
            "{col_items}",
            "<dataFields count=\"1\">",
            "<dataField name=\"{agg} of {value_header}\" fld=\"{value_field}\" subtotal=\"{subtotal}\" baseField=\"0\" baseItem=\"0\"/>",
            "</dataFields>",
            "<pivotTableStyleInfo name=\"PivotStyleLight16\" showRowHeaders=\"1\" showColHeaders=\"1\" showRowStripes=\"0\" showColStripes=\"0\" showLastColumn=\"1\"/>",
            "</pivotTableDefinition>"
        ),
        table_no = table_no,
        cache_id = plan.cache_id,
        location = xml_attr(&location),
        first_data_row = first_data_row,
        first_data_col = first_data_col,
        field_count = plan.fields.len(),
        pivot_fields = pivot_fields,
        row_field = field_index(pivot.row_field),
        row_items = row_items,
        col_field_count = u32::from(has_column_axis),
        column_fields = column_field_xml,
        col_items = col_items,
        agg = agg_label(&pivot.aggregation),
        value_header = xml_attr(&value_header),
        value_field = field_index(pivot.value_field),
        subtotal = data_subtotal(&pivot.aggregation),
    )
}

fn items_field(axis: &str, values: &[String]) -> String {
    let mut xml = format!(
        "<pivotField axis=\"{axis}\" showAll=\"0\"><items count=\"{}\">",
        values.len() + 1
    );
    for index in 0..values.len() {
        xml.push_str(&format!("<item x=\"{index}\"/>"));
    }
    xml.push_str("<item t=\"default\"/></items></pivotField>");
    xml
}

/// Adds native pivot parts to an exported XLSX package.
/// No-op when the workbook has no pivot tables.
pub fn rewrite_xlsx_with_pivots(
    bytes: Vec<u8>,
    workbook: &WorkbookModel,
) -> Result<Vec<u8>, ExportError> {
    let plans = plan_pivot_parts(workbook);
    if plans.is_empty() {
        return Ok(bytes);
    }

    let mut input = zip::ZipArchive::new(std::io::Cursor::new(bytes))
        .map_err(|error| ExportError::Xlsx(format!("could not reopen XLSX package: {error:?}")))?;

    // Part numbering continues after any existing pivot parts.
    let existing: Vec<String> = (0..input.len())
        .filter_map(|index| input.by_index(index).ok().map(|e| e.name().to_string()))
        .collect();
    let next_number = |prefix: &str| -> usize {
        existing
            .iter()
            .filter(|name| name.starts_with(prefix) && name.ends_with(".xml"))
            .filter_map(|name| {
                name.trim_start_matches(prefix)
                    .trim_end_matches(".xml")
                    .parse::<usize>()
                    .ok()
            })
            .max()
            .unwrap_or(0)
            .saturating_add(1)
    };
    let first_table_no = next_number("xl/pivotTables/pivotTable");
    let first_cache_no = next_number("xl/pivotCache/pivotCacheDefinition");

    let mut content_types = string_entry(&mut input, "[Content_Types].xml")?;
    let mut workbook_xml = string_entry(&mut input, "xl/workbook.xml")?;
    let mut workbook_rels = string_entry(&mut input, "xl/_rels/workbook.xml.rels")?;

    // --- [Content_Types].xml overrides ---
    let mut overrides = String::new();
    for index in 0..plans.len() {
        let table_no = first_table_no + index;
        let cache_no = first_cache_no + index;
        overrides.push_str(&format!(
            "<Override PartName=\"/xl/pivotTables/pivotTable{table_no}.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.pivotTable+xml\"/>"
        ));
        overrides.push_str(&format!(
            "<Override PartName=\"/xl/pivotCache/pivotCacheDefinition{cache_no}.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.pivotCacheDefinition+xml\"/>"
        ));
        overrides.push_str(&format!(
            "<Override PartName=\"/xl/pivotCache/pivotCacheRecords{cache_no}.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.pivotCacheRecords+xml\"/>"
        ));
    }
    content_types = insert_before_end(&content_types, "</Types>", &overrides)
        .ok_or_else(|| ExportError::Xlsx("malformed [Content_Types].xml".to_string()))?;

    // --- workbook relationships (cache definitions) ---
    let mut next_rel_id = next_relationship_id(&workbook_rels);
    let mut rel_ids: Vec<u32> = Vec::new();
    let mut rel_additions = String::new();
    for index in 0..plans.len() {
        let cache_no = first_cache_no + index;
        rel_additions.push_str(&format!(
            "<Relationship Id=\"rId{next_rel_id}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/pivotCacheDefinition\" Target=\"pivotCache/pivotCacheDefinition{cache_no}.xml\"/>"
        ));
        rel_ids.push(next_rel_id);
        next_rel_id = next_rel_id.saturating_add(1);
    }
    workbook_rels = insert_before_end(&workbook_rels, "</Relationships>", &rel_additions)
        .ok_or_else(|| ExportError::Xlsx("malformed workbook relationships".to_string()))?;

    // --- workbook.xml <pivotCaches> ---
    let mut caches_xml = String::from("<pivotCaches>");
    for (index, plan) in plans.iter().enumerate() {
        caches_xml.push_str(&format!(
            "<pivotCache cacheId=\"{}\" r:id=\"rId{}\"/>",
            plan.cache_id, rel_ids[index]
        ));
    }
    caches_xml.push_str("</pivotCaches>");
    workbook_xml = insert_before_end(&workbook_xml, "</workbook>", &caches_xml)
        .ok_or_else(|| ExportError::Xlsx("malformed xl/workbook.xml".to_string()))?;

    // --- worksheet rels: worksheet -> pivotTable ---
    // Group plans by owning sheet so each sheet rels file is written once.
    let mut sheet_rels_additions: HashMap<usize, Vec<String>> = HashMap::new();
    for (index, plan) in plans.iter().enumerate() {
        let table_no = first_table_no + index;
        sheet_rels_additions
            .entry(plan.sheet_index)
            .or_default()
            .push(format!(
                "<Relationship Id=\"rId{}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/pivotTable\" Target=\"../pivotTables/pivotTable{table_no}.xml\"/>",
                next_rel_id
            ));
        next_rel_id = next_rel_id.saturating_add(1);
    }
    let mut sheet_rels: HashMap<String, String> = HashMap::new();
    let mut new_sheet_rels: Vec<String> = Vec::new();
    for (sheet_index, additions) in &sheet_rels_additions {
        let rels_path = format!("xl/worksheets/_rels/sheet{}.xml.rels", sheet_index + 1);
        let joined = additions.join("");
        let existing_rels = string_entry_optional(&mut input, &rels_path);
        let xml = match existing_rels {
            Some(xml) => insert_before_end(&xml, "</Relationships>", &joined)
                .ok_or_else(|| ExportError::Xlsx(format!("malformed {rels_path}")))?,
            None => {
                new_sheet_rels.push(rels_path.clone());
                format!(
                    "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">{joined}</Relationships>"
                )
            }
        };
        sheet_rels.insert(rels_path, xml);
    }

    let mut output = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for index in 0..input.len() {
        let mut entry = input.by_index(index).map_err(|error| {
            ExportError::Xlsx(format!("could not read XLSX package entry: {error:?}"))
        })?;
        let name = entry.name().to_string();
        let compression = entry.compression();
        let modified = entry.last_modified().unwrap_or_default();
        let permissions = entry.unix_mode();
        let directory = entry.is_dir();
        let mut data = Vec::new();
        std::io::Read::read_to_end(&mut entry, &mut data).map_err(|error| {
            ExportError::Xlsx(format!("could not read XLSX package entry: {error:?}"))
        })?;
        drop(entry);

        let data: Vec<u8> = match name.as_str() {
            "[Content_Types].xml" => content_types.clone().into_bytes(),
            "xl/workbook.xml" => workbook_xml.clone().into_bytes(),
            "xl/_rels/workbook.xml.rels" => workbook_rels.clone().into_bytes(),
            other => sheet_rels
                .get(other)
                .map(|xml| xml.clone().into_bytes())
                .unwrap_or(data),
        };

        let mut options = zip::write::SimpleFileOptions::default()
            .compression_method(compression)
            .last_modified_time(modified);
        if let Some(mode) = permissions {
            options = options.unix_permissions(mode);
        }
        if directory {
            output.add_directory(&name, options).map_err(|error| {
                ExportError::Xlsx(format!("could not write XLSX entry: {error:?}"))
            })?;
        } else {
            output.start_file(&name, options).map_err(|error| {
                ExportError::Xlsx(format!("could not write XLSX entry: {error:?}"))
            })?;
            std::io::Write::write_all(&mut output, &data).map_err(|error| {
                ExportError::Xlsx(format!("could not write XLSX entry: {error:?}"))
            })?;
        }
    }

    // New worksheet rels files (sheets that had no relationships before).
    let rels_options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    for rels_path in &new_sheet_rels {
        let xml = sheet_rels
            .get(rels_path)
            .expect("new sheet rels content was just inserted");
        output
            .start_file(rels_path, rels_options)
            .map_err(|error| {
                ExportError::Xlsx(format!("could not write {rels_path}: {error:?}"))
            })?;
        std::io::Write::write_all(&mut output, xml.as_bytes()).map_err(|error| {
            ExportError::Xlsx(format!("could not write {rels_path}: {error:?}"))
        })?;
    }

    // Append the pivot parts themselves.
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    for (index, plan) in plans.iter().enumerate() {
        let table_no = first_table_no + index;
        let cache_no = first_cache_no + index;
        for (path, xml) in [
            (
                format!("xl/pivotCache/pivotCacheDefinition{cache_no}.xml"),
                cache_definition_xml(plan),
            ),
            (
                format!("xl/pivotCache/pivotCacheRecords{cache_no}.xml"),
                cache_records_xml(plan),
            ),
            (
                format!("xl/pivotTables/pivotTable{table_no}.xml"),
                pivot_table_xml(plan, table_no),
            ),
            // pivotTable part -> cacheDefinition relationship.
            (
                format!("xl/pivotTables/_rels/pivotTable{table_no}.xml.rels"),
                format!(
                    concat!(
                        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n",
                        "<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">",
                        "<Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/pivotCacheDefinition\" Target=\"../pivotCache/pivotCacheDefinition{cache_no}.xml\"/>",
                        "</Relationships>"
                    ),
                    cache_no = cache_no,
                ),
            ),
        ] {
            output
                .start_file(&path, options)
                .map_err(|error| ExportError::Xlsx(format!("could not write pivot part {path}: {error:?}")))?;
            std::io::Write::write_all(&mut output, xml.as_bytes()).map_err(|error| {
                ExportError::Xlsx(format!("could not write pivot part {path}: {error:?}"))
            })?;
        }
    }

    output
        .finish()
        .map(|cursor| cursor.into_inner())
        .map_err(|error| ExportError::Xlsx(format!("could not finish XLSX package: {error:?}")))
}

/// Import side: reads pivot parts from an XLSX archive, one list per sheet.
pub fn import_pivots_from_xlsx(
    archive: &mut zip::ZipArchive<std::io::Cursor<Vec<u8>>>,
    sheet_names: &[String],
    warnings: &mut Vec<String>,
) -> Vec<Vec<PivotTableModel>> {
    let mut per_sheet: Vec<Vec<PivotTableModel>> = sheet_names.iter().map(|_| Vec::new()).collect();
    let Some(workbook_xml) = read_entry(archive, "xl/workbook.xml") else {
        return per_sheet;
    };
    let caches = parse_workbook_pivot_caches(&workbook_xml);
    if caches.is_empty() {
        return per_sheet;
    }
    let Some(rels_xml) = read_entry(archive, "xl/_rels/workbook.xml.rels") else {
        return per_sheet;
    };
    let relationships = parse_relationship_map(&rels_xml);
    let cache_definitions: HashMap<u32, (String, String)> = caches
        .iter()
        .filter_map(|(cache_id, rel_id)| {
            relationships
                .get(rel_id)
                .map(|target| (*cache_id, (normalize_target("xl", target), rel_id.clone())))
        })
        .filter(|(_, (path, _))| path.starts_with("xl/pivotCache/"))
        .collect();

    for (cache_id, (definition_path, _)) in &cache_definitions {
        let Some(definition_xml) = read_entry(archive, definition_path) else {
            warnings.push(format!("pivot cache {cache_id} definition was unreadable"));
            continue;
        };
        let Some(cache) = parse_cache_definition(&definition_xml, *cache_id) else {
            warnings.push(format!("pivot cache {cache_id} definition was unsupported"));
            continue;
        };

        // Find the pivotTable parts that reference this cache (via their rels)
        // and map each to its owning sheet through the worksheet rels.
        let mut assigned = false;
        for (sheet_index, _) in sheet_names.iter().enumerate() {
            let rels_path = format!("xl/worksheets/_rels/sheet{}.xml.rels", sheet_index + 1);
            let Some(rels) = read_entry(archive, &rels_path) else {
                continue;
            };
            let table_targets: Vec<String> = parse_relationship_map(&rels)
                .into_iter()
                .filter(|(_, target)| {
                    normalize_target("xl/worksheets", target)
                        .starts_with("xl/pivotTables/pivotTable")
                })
                .map(|(_, target)| normalize_target("xl/worksheets", &target))
                .collect();
            for table_path in table_targets {
                let Some(table_xml) = read_entry(archive, &table_path) else {
                    continue;
                };
                if let Some(mut pivot) = parse_pivot_table(&table_xml, cache.clone()) {
                    if let Some(sheet) = per_sheet.get_mut(sheet_index) {
                        if sheet.len() < MAX_PIVOT_PARTS {
                            pivot.id = format!("xlsx-pivot-{}-{}", cache_id, sheet.len() + 1);
                            sheet.push(pivot);
                            assigned = true;
                        }
                    }
                }
            }
            if assigned {
                break;
            }
        }
    }
    per_sheet
}

#[derive(Clone)]
struct CacheContext {
    source_range: CellRange,
}

fn parse_cache_definition(xml: &[u8], _cache_id: u32) -> Option<CacheContext> {
    let mut reader = quick_xml::Reader::from_reader(xml);
    reader.config_mut().trim_text(true);
    let mut buffer = Vec::new();
    let mut source_ref = None;
    while let Ok(event) = reader.read_event_into(&mut buffer) {
        if let quick_xml::events::Event::Empty(element) = &event {
            if element.name().as_ref() == b"worksheetSource" {
                for attribute in element.attributes().flatten() {
                    if attribute.key.as_ref() == b"ref" {
                        source_ref =
                            Some(String::from_utf8_lossy(attribute.value.as_ref()).into_owned());
                    }
                }
            }
        }
        if matches!(event, quick_xml::events::Event::Eof) {
            break;
        }
        buffer.clear();
    }
    let source_ref = source_ref?;
    let source_range = parse_range_ref(&source_ref)?;
    Some(CacheContext { source_range })
}

fn parse_pivot_table(xml: &[u8], cache: CacheContext) -> Option<PivotTableModel> {
    let mut reader = quick_xml::Reader::from_reader(xml);
    reader.config_mut().trim_text(true);
    let mut buffer = Vec::new();
    let mut location_ref = None;
    let mut row_field = None;
    let mut column_field = None;
    let mut value_field = None;
    let mut aggregation = "sum".to_string();
    let mut current_section = None::<&'static str>;
    while let Ok(event) = reader.read_event_into(&mut buffer) {
        match &event {
            quick_xml::events::Event::Empty(element) | quick_xml::events::Event::Start(element) => {
                let local = element.name().as_ref().to_vec();
                match local.as_slice() {
                    b"location" => {
                        location_ref = element
                            .attributes()
                            .flatten()
                            .find(|attribute| attribute.key.as_ref() == b"ref")
                            .map(|attribute| {
                                String::from_utf8_lossy(attribute.value.as_ref()).into_owned()
                            });
                    }
                    b"rowFields" => current_section = Some("row"),
                    b"colFields" => current_section = Some("col"),
                    b"dataFields" => current_section = Some("data"),
                    b"field" => {
                        let x = element
                            .attributes()
                            .flatten()
                            .find(|attribute| attribute.key.as_ref() == b"x")
                            .and_then(|attribute| {
                                String::from_utf8_lossy(attribute.value.as_ref())
                                    .parse::<u32>()
                                    .ok()
                            });
                        if let Some(x) = x {
                            match current_section {
                                Some("row") => row_field = Some(x),
                                Some("col") => column_field = Some(x),
                                _ => {}
                            }
                        }
                    }
                    b"dataField" => {
                        let fld = element
                            .attributes()
                            .flatten()
                            .find(|attribute| attribute.key.as_ref() == b"fld")
                            .and_then(|attribute| {
                                String::from_utf8_lossy(attribute.value.as_ref())
                                    .parse::<u32>()
                                    .ok()
                            });
                        let subtotal = element
                            .attributes()
                            .flatten()
                            .find(|attribute| attribute.key.as_ref() == b"subtotal")
                            .map(|attribute| {
                                String::from_utf8_lossy(attribute.value.as_ref()).into_owned()
                            })
                            .unwrap_or_else(|| "2".to_string());
                        aggregation = match subtotal.trim() {
                            "64" => "count".to_string(),
                            "1024" => "average".to_string(),
                            _ => "sum".to_string(),
                        };
                        if let Some(fld) = fld {
                            value_field = Some(fld);
                        }
                    }
                    _ => {}
                }
            }
            quick_xml::events::Event::End(element) => match element.name().as_ref() {
                b"rowFields" | b"colFields" | b"dataFields" => current_section = None,
                _ => {}
            },
            quick_xml::events::Event::Eof => break,
            _ => {}
        }
        buffer.clear();
    }

    let row_field = row_field?;
    let value_field = value_field.unwrap_or(row_field);
    let start_col = cache.source_range.start_col;
    let (output_row, output_col) = location_ref
        .as_deref()
        .and_then(parse_range_ref)
        .map(|range| (range.start_row, range.start_col))
        .unwrap_or((1, 1));

    Some(PivotTableModel {
        id: "xlsx-pivot".to_string(),
        source_range: cache.source_range,
        row_field: start_col.saturating_add(row_field),
        column_field: column_field.map(|field| start_col.saturating_add(field)),
        value_field: start_col.saturating_add(value_field),
        aggregation,
        output_start_row: output_row,
        output_start_col: output_col,
        output_row_count: None,
        output_col_count: None,
    })
}

fn parse_range_ref(reference: &str) -> Option<CellRange> {
    let reference = reference.trim().trim_start_matches('=');
    let range = reference
        .rsplit_once('!')
        .map(|(_, range)| range)
        .unwrap_or(reference)
        .replace('$', "");
    let (start, end) = range
        .split_once(':')
        .unwrap_or((range.as_str(), range.as_str()));
    let (start_col, start_row) = parse_a1(start)?;
    let (end_col, end_row) = parse_a1(end)?;
    Some(CellRange {
        start_row,
        end_row,
        start_col,
        end_col,
    })
}

fn parse_a1(cell: &str) -> Option<(u32, u32)> {
    let cell = cell.trim();
    let split = cell
        .find(|c: char| c.is_ascii_digit())
        .unwrap_or(cell.len());
    if split == 0 || split == cell.len() {
        return None;
    }
    let mut column = 0u32;
    for character in cell[..split].chars() {
        let letter = character.to_ascii_uppercase();
        if !letter.is_ascii_uppercase() {
            return None;
        }
        column = column
            .checked_mul(26)?
            .checked_add(u32::from(letter as u8 - b'A' + 1))?;
    }
    let row = cell[split..].parse::<u32>().ok()?;
    (row > 0 && column > 0).then_some((column, row))
}

fn parse_workbook_pivot_caches(workbook_xml: &[u8]) -> Vec<(u32, String)> {
    let mut reader = quick_xml::Reader::from_reader(workbook_xml);
    reader.config_mut().trim_text(true);
    let mut buffer = Vec::new();
    let mut caches = Vec::new();
    while let Ok(event) = reader.read_event_into(&mut buffer) {
        if let quick_xml::events::Event::Empty(element) = &event {
            if element.name().as_ref() == b"pivotCache" {
                let mut id = None;
                let mut rel = None;
                for attribute in element.attributes().flatten() {
                    let value = String::from_utf8_lossy(attribute.value.as_ref()).into_owned();
                    match attribute.key.as_ref() {
                        b"cacheId" => id = value.parse::<u32>().ok(),
                        b"id" | b"r:id" => rel = Some(value),
                        _ => {}
                    }
                }
                if let (Some(id), Some(rel)) = (id, rel) {
                    caches.push((id, rel));
                }
            }
        }
        if matches!(event, quick_xml::events::Event::Eof) {
            break;
        }
        buffer.clear();
    }
    caches
}

fn parse_relationship_map(xml: &[u8]) -> HashMap<String, String> {
    let mut reader = quick_xml::Reader::from_reader(xml);
    reader.config_mut().trim_text(true);
    let mut buffer = Vec::new();
    let mut map = HashMap::new();
    while let Ok(event) = reader.read_event_into(&mut buffer) {
        if let quick_xml::events::Event::Empty(element) | quick_xml::events::Event::Start(element) =
            &event
        {
            if element.name().as_ref() == b"Relationship" {
                let mut id = None;
                let mut target = None;
                for attribute in element.attributes().flatten() {
                    match attribute.key.as_ref() {
                        b"Id" => {
                            id =
                                Some(String::from_utf8_lossy(attribute.value.as_ref()).into_owned())
                        }
                        b"Target" => {
                            target =
                                Some(String::from_utf8_lossy(attribute.value.as_ref()).into_owned())
                        }
                        _ => {}
                    }
                }
                if let (Some(id), Some(target)) = (id, target) {
                    map.insert(id, target);
                }
            }
        }
        if matches!(event, quick_xml::events::Event::Eof) {
            break;
        }
        buffer.clear();
    }
    map
}

fn normalize_target(base_dir: &str, target: &str) -> String {
    let mut parts: Vec<String> = base_dir
        .split('/')
        .filter(|part| !part.is_empty())
        .map(str::to_string)
        .collect();
    for part in target.trim_start_matches('/').split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            value => parts.push(value.to_string()),
        }
    }
    parts.join("/")
}

fn read_entry(
    archive: &mut zip::ZipArchive<std::io::Cursor<Vec<u8>>>,
    path: &str,
) -> Option<Vec<u8>> {
    let mut file = archive.by_name(path).ok()?;
    let mut data = Vec::new();
    std::io::Read::read_to_end(&mut file, &mut data).ok()?;
    Some(data)
}

fn string_entry(
    archive: &mut zip::ZipArchive<std::io::Cursor<Vec<u8>>>,
    path: &str,
) -> Result<String, ExportError> {
    read_entry(archive, path)
        .and_then(|xml| String::from_utf8(xml).ok())
        .ok_or_else(|| ExportError::Xlsx(format!("missing or invalid {path}")))
}

fn string_entry_optional(
    archive: &mut zip::ZipArchive<std::io::Cursor<Vec<u8>>>,
    path: &str,
) -> Option<String> {
    read_entry(archive, path).and_then(|xml| String::from_utf8(xml).ok())
}

fn insert_before_end(xml: &str, end_marker: &str, addition: &str) -> Option<String> {
    let position = xml.rfind(end_marker)?;
    let mut result = String::with_capacity(xml.len() + addition.len());
    result.push_str(&xml[..position]);
    result.push_str(addition);
    result.push_str(&xml[position..]);
    Some(result)
}

fn next_relationship_id(rels_xml: &str) -> u32 {
    rels_xml
        .match_indices("Id=\"rId")
        .filter_map(|(offset, _)| {
            let rest = &rels_xml[offset + 8..];
            let end = rest.find('"')?;
            rest[..end].parse::<u32>().ok()
        })
        .max()
        .unwrap_or(0)
        .saturating_add(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use redoc_sheet_engine::SheetCell;

    fn sheet_with_source() -> SheetData {
        let mut sheet = SheetData::new("sheet-1", "Data");
        for (index, label) in ["Region", "Product", "Sales"].iter().enumerate() {
            sheet.cells.insert(
                format!("1:{}", index + 1),
                SheetCell {
                    raw_value: label.to_string(),
                    display_value: label.to_string(),
                    formula: None,
                    style: None,
                },
            );
        }
        let rows = [
            ("East", "A", "10"),
            ("West", "B", "20"),
            ("East", "B", "30"),
        ];
        for (row_index, (region, product, sales)) in rows.iter().enumerate() {
            let row = (row_index + 2).to_string();
            for (col, value) in [region, product, sales].iter().enumerate() {
                sheet.cells.insert(
                    format!("{row}:{}", col + 1),
                    SheetCell {
                        raw_value: value.to_string(),
                        display_value: value.to_string(),
                        formula: None,
                        style: None,
                    },
                );
            }
        }
        sheet
    }

    #[test]
    fn writes_native_pivot_parts_round_trip() {
        let mut workbook = WorkbookModel::new_default();
        let mut sheet = sheet_with_source();
        sheet.pivot_tables.push(PivotTableModel {
            id: "pivot-1".to_string(),
            source_range: CellRange {
                start_row: 1,
                end_row: 4,
                start_col: 1,
                end_col: 3,
            },
            row_field: 1,
            column_field: Some(2),
            value_field: 3,
            aggregation: "sum".to_string(),
            output_start_row: 7,
            output_start_col: 1,
            output_row_count: Some(4),
            output_col_count: Some(4),
        });
        workbook.sheets[0] = sheet;

        let bytes = crate::xlsx::export_workbook_to_xlsx(&workbook).expect("export xlsx");
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).expect("read xlsx zip");

        let definition = read_text(&mut archive, "xl/pivotCache/pivotCacheDefinition1.xml");
        assert!(definition.contains("recordCount=\"3\""));
        assert!(definition.contains("refreshOnLoad=\"1\""));
        assert!(definition.contains("name=\"Region\""));
        assert!(definition.contains("dataField=\"1\""));

        let records = read_text(&mut archive, "xl/pivotCache/pivotCacheRecords1.xml");
        assert!(records.contains("<n v=\"10\"/>"));
        assert!(records.contains("<s v=\"East\"/>"));

        let table = read_text(&mut archive, "xl/pivotTables/pivotTable1.xml");
        assert!(table.contains("<rowFields count=\"1\">"));
        assert!(table.contains("Sum of Sales"));
        // pivotFields stay in cacheField order: row(axis1), column(axis2), value.
        let row_axis_pos = table.find("axis=\"axis1\"").expect("row axis");
        let col_axis_pos = table.find("axis=\"axis2\"").expect("col axis");
        assert!(row_axis_pos < col_axis_pos);

        let workbook_xml = read_text(&mut archive, "xl/workbook.xml");
        assert!(workbook_xml.contains("<pivotCaches>"));
        assert!(workbook_xml.contains("cacheId=\"1\""));

        let content_types = read_text(&mut archive, "[Content_Types].xml");
        assert!(content_types.contains("pivotTable+xml"));

        let sheet_rels = read_text(&mut archive, "xl/worksheets/_rels/sheet1.xml.rels");
        assert!(sheet_rels.contains("pivotTable1.xml"));

        let table_rels = read_text(&mut archive, "xl/pivotTables/_rels/pivotTable1.xml.rels");
        assert!(table_rels.contains("pivotCacheDefinition1.xml"));

        // Round-trip: the parts parse back into a PivotTableModel.
        let mut warnings = Vec::new();
        let per_sheet = import_pivots_from_xlsx(&mut archive, &["Data".to_string()], &mut warnings);
        assert!(warnings.is_empty());
        let pivots = &per_sheet[0];
        assert_eq!(pivots.len(), 1);
        assert_eq!(pivots[0].row_field, 1);
        assert_eq!(pivots[0].column_field, Some(2));
        assert_eq!(pivots[0].value_field, 3);
        assert_eq!(pivots[0].aggregation, "sum");
        assert_eq!(pivots[0].output_start_row, 7);
        assert_eq!(pivots[0].output_start_col, 1);
    }

    #[test]
    fn skips_workbooks_without_pivots() {
        let workbook = WorkbookModel::new_default();
        let bytes = vec![1u8, 2, 3];
        let result = rewrite_xlsx_with_pivots(bytes, &workbook).expect("no-op");
        assert_eq!(result, vec![1u8, 2, 3]);
    }

    #[test]
    fn parses_a1_references() {
        assert_eq!(parse_a1("B7"), Some((2, 7)));
        assert_eq!(parse_a1("AA3"), Some((27, 3)));
        assert_eq!(parse_a1("7"), None);
        let range = parse_range_ref("'My Sheet'!A1:C4").expect("range");
        assert_eq!(
            (
                range.start_row,
                range.start_col,
                range.end_row,
                range.end_col
            ),
            (1, 1, 4, 3)
        );
    }

    fn read_text(archive: &mut zip::ZipArchive<std::io::Cursor<Vec<u8>>>, path: &str) -> String {
        let mut file = archive
            .by_name(path)
            .unwrap_or_else(|error| panic!("missing {path}: {error:?}"));
        let mut xml = String::new();
        std::io::Read::read_to_string(&mut file, &mut xml).expect("read utf-8");
        xml
    }
}
