use crate::base64_util::{data_uri_bytes, encode_base64};
use crate::pdf::ExportError;
use calamine::{open_workbook_auto, Data, Reader};
use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader as XmlReader;
use redoc_sheet_engine::workbook::NamedRange;
use redoc_sheet_engine::{
    AutoFilterState, CellStyle, ChartModel, ConditionalFormattingRule, ConditionalFormattingStyle,
    ListValidation, MergeRange, ScenarioModel, SheetCell, SheetData, WorkbookModel,
};
use rust_xlsxwriter::{
    Chart, ChartType, Color, ConditionalFormat2ColorScale, ConditionalFormat3ColorScale,
    ConditionalFormatCell, ConditionalFormatCellRule, ConditionalFormatDataBar,
    ConditionalFormatText, ConditionalFormatTextRule, DataValidation, FilterCondition, Format,
    FormatAlign, FormatBorder, FormatUnderline, Image, Table, TableColumn, TableStyle, Workbook,
};
use std::collections::{BTreeMap, HashMap};
use std::io::{Cursor, Read, Write};
use std::path::Path;

const MAX_INLINE_IMAGE_BYTES: usize = 8 * 1024 * 1024;
const MAX_TOTAL_INLINE_IMAGE_BYTES: u64 = 64 * 1024 * 1024;

fn xlsx_image_bytes(value: &str) -> Option<Vec<u8>> {
    let (header, _) = value.split_once(',')?;
    let mime = header
        .strip_prefix("data:")?
        .split(';')
        .next()?
        .trim()
        .to_ascii_lowercase();
    if !matches!(mime.as_str(), "image/png" | "image/jpeg" | "image/jpg") {
        return None;
    }
    let bytes = data_uri_bytes(value)?;
    (bytes.len() <= MAX_INLINE_IMAGE_BYTES).then_some(bytes)
}

#[derive(Debug)]
pub struct XlsxImportResult {
    pub workbook: WorkbookModel,
    pub warnings: Vec<String>,
}

#[derive(Default)]
struct ImportedXlsxMetadata {
    cell_style_ids: HashMap<String, usize>,
    col_widths: BTreeMap<u32, f64>,
    row_heights: BTreeMap<u32, f64>,
    freeze_rows: u32,
    freeze_cols: u32,
    styles: Vec<ImportedXf>,
    conditional_formatting: Vec<ConditionalFormattingRule>,
    auto_filter: Option<AutoFilterState>,
    merges: Vec<MergeRange>,
    validations: BTreeMap<String, ListValidation>,
    hyperlinks: BTreeMap<String, String>,
    tables: Vec<redoc_sheet_engine::TableModel>,
    scenarios: Vec<ScenarioModel>,
}

#[derive(Default)]
struct ImportedXf {
    num_format_id: u32,
    format_code: Option<String>,
    alignment: Option<String>,
    font_id: usize,
    fill_id: usize,
    border_id: usize,
    bold: Option<bool>,
    italic: Option<bool>,
    underline: Option<bool>,
    font_color: Option<String>,
    bg_color: Option<String>,
    font_family: Option<String>,
    font_size: Option<f64>,
    decimals: Option<u32>,
    borders: Option<redoc_sheet_engine::CellBorders>,
}

#[derive(Default)]
struct ImportedFont {
    bold: bool,
    italic: bool,
    underline: bool,
    color: Option<String>,
    name: Option<String>,
    size: Option<f64>,
}

#[derive(Default)]
struct ImportedFill {
    color: Option<String>,
}

#[derive(Default, Clone)]
struct ImportedBorderEdge {
    style: Option<String>,
    color: Option<String>,
}

#[derive(Default, Clone)]
struct ImportedBorder {
    top: ImportedBorderEdge,
    right: ImportedBorderEdge,
    bottom: ImportedBorderEdge,
    left: ImportedBorderEdge,
}

fn normalize_rgb(value: &str) -> Option<String> {
    let value = value.trim().trim_start_matches('#');
    let value = if value.len() == 8 { &value[2..] } else { value };
    (value.len() == 6).then(|| format!("#{value}"))
}

fn xml_attr(element: &BytesStart<'_>, name: &[u8]) -> Option<String> {
    element
        .attributes()
        .flatten()
        .find(|attribute| attribute.key.as_ref() == name)
        .and_then(|attribute| String::from_utf8(attribute.value.into_owned()).ok())
}

fn parse_xlsx_styles(archive: &mut zip::ZipArchive<std::io::Cursor<Vec<u8>>>) -> Vec<ImportedXf> {
    let Ok(mut file) = archive.by_name("xl/styles.xml") else {
        return Vec::new();
    };
    if file.size() > MAX_XLSX_XML_BYTES {
        return Vec::new();
    }
    let mut xml = Vec::new();
    if file.read_to_end(&mut xml).is_err() {
        return Vec::new();
    }
    let mut reader = XmlReader::from_reader(xml.as_slice());
    reader.config_mut().trim_text(true);
    let mut styles = Vec::new();
    let mut custom_formats: HashMap<u32, String> = HashMap::new();
    let mut in_cell_xfs = false;
    let mut current: Option<ImportedXf> = None;
    let mut in_fonts = false;
    let mut fonts = Vec::new();
    let mut current_font: Option<ImportedFont> = None;
    let mut in_fills = false;
    let mut fills = Vec::new();
    let mut current_fill: Option<ImportedFill> = None;
    // Border parsing: <borders><border><left style=".."><color rgb=".."/>
    let mut in_borders = false;
    let mut borders = Vec::new();
    let mut current_border: Option<ImportedBorder> = None;
    let mut current_border_side: Option<u8> = None; // 0=top 1=right 2=bottom 3=left
    let mut buffer = Vec::new();
    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Empty(element)) if element.name().as_ref() == b"numFmt" => {
                if let (Some(id), Some(code)) = (
                    xml_attr(&element, b"numFmtId").and_then(|value| value.parse().ok()),
                    xml_attr(&element, b"formatCode"),
                ) {
                    custom_formats.insert(id, code);
                }
            }
            Ok(Event::Start(element)) if element.name().as_ref() == b"fonts" => {
                in_fonts = true;
            }
            Ok(Event::End(element)) if element.name().as_ref() == b"fonts" => {
                in_fonts = false;
            }
            Ok(Event::Start(element)) if in_fonts && element.name().as_ref() == b"font" => {
                current_font = Some(ImportedFont::default());
            }
            Ok(Event::End(element))
                if current_font.is_some() && element.name().as_ref() == b"font" =>
            {
                if let Some(font) = current_font.take() {
                    fonts.push(font);
                }
            }
            Ok(Event::Empty(element))
                if current_font.is_some() && element.name().as_ref() == b"b" =>
            {
                if let Some(font) = current_font.as_mut() {
                    font.bold = true;
                }
            }
            Ok(Event::Empty(element))
                if current_font.is_some() && element.name().as_ref() == b"i" =>
            {
                if let Some(font) = current_font.as_mut() {
                    font.italic = true;
                }
            }
            Ok(Event::Empty(element))
                if current_font.is_some() && element.name().as_ref() == b"u" =>
            {
                if let Some(font) = current_font.as_mut() {
                    font.underline = true;
                }
            }
            Ok(Event::Empty(element))
                if current_font.is_some() && element.name().as_ref() == b"color" =>
            {
                if let Some(font) = current_font.as_mut() {
                    font.color = xml_attr(&element, b"rgb").and_then(|value| normalize_rgb(&value));
                }
            }
            Ok(Event::Empty(element))
                if current_font.is_some() && element.name().as_ref() == b"sz" =>
            {
                if let Some(font) = current_font.as_mut() {
                    font.size = xml_attr(&element, b"val").and_then(|value| value.parse().ok());
                }
            }
            Ok(Event::Empty(element)) | Ok(Event::Start(element))
                if current_font.is_some() && element.name().as_ref() == b"name" =>
            {
                if let Some(font) = current_font.as_mut() {
                    if let Some(name) = xml_attr(&element, b"val") {
                        font.name = Some(name);
                    }
                }
            }
            Ok(Event::Start(element)) if element.name().as_ref() == b"fills" => {
                in_fills = true;
            }
            Ok(Event::End(element)) if element.name().as_ref() == b"fills" => {
                in_fills = false;
            }
            Ok(Event::Start(element)) if in_fills && element.name().as_ref() == b"fill" => {
                current_fill = Some(ImportedFill::default());
            }
            Ok(Event::End(element))
                if current_fill.is_some() && element.name().as_ref() == b"fill" =>
            {
                if let Some(fill) = current_fill.take() {
                    fills.push(fill);
                }
            }
            Ok(Event::Empty(element))
                if current_fill.is_some() && element.name().as_ref() == b"fgColor" =>
            {
                if let Some(fill) = current_fill.as_mut() {
                    fill.color = xml_attr(&element, b"rgb").and_then(|value| normalize_rgb(&value));
                }
            }
            Ok(Event::Start(element)) if element.name().as_ref() == b"borders" => {
                in_borders = true;
            }
            Ok(Event::End(element)) if element.name().as_ref() == b"borders" => {
                in_borders = false;
            }
            Ok(Event::Start(element)) if in_borders && element.name().as_ref() == b"border" => {
                current_border = Some(ImportedBorder::default());
            }
            Ok(Event::End(element))
                if current_border.is_some() && element.name().as_ref() == b"border" =>
            {
                if let Some(border) = current_border.take() {
                    borders.push(border);
                }
                current_border_side = None;
            }
            Ok(Event::Start(element)) | Ok(Event::Empty(element))
                if current_border.is_some() && matches!(element.name().as_ref(), b"top" | b"right" | b"bottom" | b"left") =>
            {
                let side = match element.name().as_ref() {
                    b"top" => 0u8,
                    b"right" => 1u8,
                    b"bottom" => 2u8,
                    _ => 3u8,
                };
                current_border_side = Some(side);
                let style = xml_attr(&element, b"style");
                if let Some(border) = current_border.as_mut() {
                    let edge = match side {
                        0 => &mut border.top,
                        1 => &mut border.right,
                        2 => &mut border.bottom,
                        _ => &mut border.left,
                    };
                    edge.style = style;
                }
            }
            Ok(Event::End(element))
                if current_border.is_some() && matches!(element.name().as_ref(), b"top" | b"right" | b"bottom" | b"left") =>
            {
                current_border_side = None;
            }
            Ok(Event::Empty(element))
                if current_border.is_some() && element.name().as_ref() == b"color" =>
            {
                let rgb = xml_attr(&element, b"rgb").and_then(|value| normalize_rgb(&value));
                if let (Some(side), Some(border)) = (current_border_side, current_border.as_mut()) {
                    let edge = match side {
                        0 => &mut border.top,
                        1 => &mut border.right,
                        2 => &mut border.bottom,
                        _ => &mut border.left,
                    };
                    edge.color = rgb;
                }
            }
            Ok(Event::Start(element)) if element.name().as_ref() == b"cellXfs" => {
                in_cell_xfs = true;
            }
            Ok(Event::End(element)) if element.name().as_ref() == b"cellXfs" => {
                in_cell_xfs = false;
            }
            Ok(Event::Empty(element)) if in_cell_xfs && element.name().as_ref() == b"xf" => {
                let num_format_id = xml_attr(&element, b"numFmtId")
                    .and_then(|value| value.parse().ok())
                    .unwrap_or_default();
                styles.push(ImportedXf {
                    num_format_id,
                    format_code: custom_formats.get(&num_format_id).cloned(),
                    alignment: None,
                    font_id: xml_attr(&element, b"fontId")
                        .and_then(|value| value.parse().ok())
                        .unwrap_or_default(),
                    fill_id: xml_attr(&element, b"fillId")
                        .and_then(|value| value.parse().ok())
                        .unwrap_or_default(),
                    border_id: xml_attr(&element, b"borderId")
                        .and_then(|value| value.parse().ok())
                        .unwrap_or_default(),
                    ..Default::default()
                });
            }
            Ok(Event::Start(element)) if in_cell_xfs && element.name().as_ref() == b"xf" => {
                let num_format_id = xml_attr(&element, b"numFmtId")
                    .and_then(|value| value.parse().ok())
                    .unwrap_or_default();
                current = Some(ImportedXf {
                    num_format_id,
                    format_code: custom_formats.get(&num_format_id).cloned(),
                    alignment: None,
                    font_id: xml_attr(&element, b"fontId")
                        .and_then(|value| value.parse().ok())
                        .unwrap_or_default(),
                    fill_id: xml_attr(&element, b"fillId")
                        .and_then(|value| value.parse().ok())
                        .unwrap_or_default(),
                    border_id: xml_attr(&element, b"borderId")
                        .and_then(|value| value.parse().ok())
                        .unwrap_or_default(),
                    ..Default::default()
                });
            }
            Ok(Event::Empty(element))
                if current.is_some() && element.name().as_ref() == b"alignment" =>
            {
                if let Some(style) = current.as_mut() {
                    style.alignment = xml_attr(&element, b"horizontal");
                }
            }
            Ok(Event::End(element)) if current.is_some() && element.name().as_ref() == b"xf" => {
                if let Some(style) = current.take() {
                    styles.push(style);
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
        buffer.clear();
    }
    for style in &mut styles {
        if let Some(font) = fonts.get(style.font_id) {
            style.bold = font.bold.then_some(true);
            style.italic = font.italic.then_some(true);
            style.underline = font.underline.then_some(true);
            style.font_color = font.color.clone();
            style.font_family = font.name.clone();
            style.font_size = font.size;
        }
        if let Some(fill) = fills.get(style.fill_id) {
            style.bg_color = fill.color.clone();
        }
        if let Some(code) = style.format_code.as_deref() {
            style.decimals = decimals_in_format_code(code);
        }
        if let Some(border) = borders.get(style.border_id) {
            let edge = |edge: &ImportedBorderEdge| {
                let style = edge.style.as_deref()?;
                // Excel has more border styles than the bounded Redoc set;
                // approximate exotic ones rather than dropping them.
                let normalized = match style {
                    "thin" | "hair" => "thin",
                    "medium" | "mediumDashDot" | "mediumDashDotDot" | "mediumDashed" => "medium",
                    "thick" => "thick",
                    "dashed" | "dashDot" | "dashDotDot" | "slantDashDot" => "dashed",
                    "dotted" => "dotted",
                    "double" => "double",
                    _ => "thin",
                };
                Some(redoc_sheet_engine::BorderEdge {
                    style: normalized.to_string(),
                    color: edge.color.clone(),
                })
            };
            let borders = redoc_sheet_engine::CellBorders {
                top: edge(&border.top),
                right: edge(&border.right),
                bottom: edge(&border.bottom),
                left: edge(&border.left),
            };
            style.borders = (borders.top.is_some()
                || borders.right.is_some()
                || borders.bottom.is_some()
                || borders.left.is_some())
            .then_some(borders);
        }
    }
    styles
}

/// Count the digits after '.' in the numeric section of an Excel format code.
/// `"#,##0.00"` → 2; `"$#,##0"` → None (no decimal section).
fn decimals_in_format_code(code: &str) -> Option<u32> {
    let relevant = code.split(';').next().unwrap_or(code);
    // Skip quoted literals and percent scaling; just count 0s after the dot.
    let after_dot = relevant.split('.').nth(1)?;
    let mut zeros = 0u32;
    for c in after_dot.chars() {
        if c.is_ascii_digit() || c == '#' || c == '0' {
            if c == '0' {
                zeros += 1;
            }
        } else {
            // Quoted literals, escapes, and any other punctuation end the scan.
            break;
        }
    }
    (zeros > 0).then_some(zeros)
}

fn parse_xlsx_dxf_styles(
    archive: &mut zip::ZipArchive<std::io::Cursor<Vec<u8>>>,
) -> Vec<ConditionalFormattingStyle> {
    let Ok(mut file) = archive.by_name("xl/styles.xml") else {
        return Vec::new();
    };
    if file.size() > MAX_XLSX_XML_BYTES {
        return Vec::new();
    }
    let mut xml = Vec::new();
    if file.read_to_end(&mut xml).is_err() {
        return Vec::new();
    }
    let mut reader = XmlReader::from_reader(xml.as_slice());
    reader.config_mut().trim_text(true);
    let mut buffer = Vec::new();
    let mut in_dxfs = false;
    let mut in_dxf = false;
    let mut in_font = false;
    let mut in_fill = false;
    let mut current = None::<ConditionalFormattingStyle>;
    let mut styles = Vec::new();
    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(element)) => match xml_local_name(element.name().as_ref()) {
                b"dxfs" => in_dxfs = true,
                b"dxf" if in_dxfs => {
                    in_dxf = true;
                    current = Some(ConditionalFormattingStyle {
                        font_color: None,
                        bg_color: None,
                        bold: None,
                        italic: None,
                    });
                }
                b"font" if in_dxf => in_font = true,
                b"fill" if in_dxf => in_fill = true,
                _ => {}
            },
            Ok(Event::Empty(element)) => {
                let name = xml_local_name(element.name().as_ref()).to_vec();
                if let Some(style) = current.as_mut() {
                    match name.as_slice() {
                        b"b" if in_font => style.bold = Some(true),
                        b"i" if in_font => style.italic = Some(true),
                        b"color" if in_font => {
                            style.font_color = xml_attr_local(&element, b"rgb")
                                .and_then(|value| normalize_rgb(&value));
                        }
                        b"fgColor" | b"bgColor" if in_fill => {
                            style.bg_color = xml_attr_local(&element, b"rgb")
                                .and_then(|value| normalize_rgb(&value));
                        }
                        _ => {}
                    }
                }
            }
            Ok(Event::End(element)) => match xml_local_name(element.name().as_ref()) {
                b"font" => in_font = false,
                b"fill" => in_fill = false,
                b"dxf" if in_dxf => {
                    if let Some(style) = current.take() {
                        styles.push(style);
                    }
                    in_dxf = false;
                }
                b"dxfs" => in_dxfs = false,
                _ => {}
            },
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
        buffer.clear();
    }
    styles
}

fn parse_xlsx_sheet_metadata(
    archive: &mut zip::ZipArchive<std::io::Cursor<Vec<u8>>>,
    sheet_index: usize,
) -> ImportedXlsxMetadata {
    let mut metadata = ImportedXlsxMetadata {
        styles: parse_xlsx_styles(archive),
        ..Default::default()
    };
    let dxf_styles = parse_xlsx_dxf_styles(archive);
    let path = format!("xl/worksheets/sheet{}.xml", sheet_index + 1);
    let sheet_relationships = relationship_part_path(&path)
        .and_then(|relationships_path| read_xlsx_entry(archive, &relationships_path))
        .map(|xml| parse_relationships(&xml))
        .unwrap_or_default();
    let Ok(mut file) = archive.by_name(&path) else {
        return metadata;
    };
    let mut xml = Vec::new();
    if file.read_to_end(&mut xml).is_err() {
        return metadata;
    }
    drop(file);
    let mut reader = XmlReader::from_reader(xml.as_slice());
    reader.config_mut().trim_text(true);
    let mut buffer = Vec::new();
    let mut conditional_range = None;
    let mut current_conditional = None::<ImportedConditionalRule>;
    let mut current_filter_column = None::<u32>;
    let mut current_validation = None::<ImportedValidation>;
    let mut capturing_formula = false;
    let mut formula_text = String::new();
    let mut capturing_validation_formula = false;
    let mut validation_formula_text = String::new();
    let mut table_part_ids = Vec::new();
    let mut current_scenario = None::<ScenarioModel>;
    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(element))
                if xml_local_name(element.name().as_ref()) == b"autoFilter" =>
            {
                if let Some(range) =
                    xml_attr_local(&element, b"ref").and_then(|value| parse_excel_range(&value))
                {
                    metadata.auto_filter = Some(AutoFilterState {
                        enabled: true,
                        start_row: range.start_row,
                        end_row: range.end_row,
                        start_col: range.start_col,
                        end_col: range.end_col,
                        column_filters: BTreeMap::new(),
                    });
                }
            }
            Ok(Event::Empty(element))
                if xml_local_name(element.name().as_ref()) == b"autoFilter" =>
            {
                if let Some(range) =
                    xml_attr_local(&element, b"ref").and_then(|value| parse_excel_range(&value))
                {
                    metadata.auto_filter = Some(AutoFilterState {
                        enabled: true,
                        start_row: range.start_row,
                        end_row: range.end_row,
                        start_col: range.start_col,
                        end_col: range.end_col,
                        column_filters: BTreeMap::new(),
                    });
                }
            }
            Ok(Event::Start(element))
                if xml_local_name(element.name().as_ref()) == b"filterColumn" =>
            {
                current_filter_column =
                    xml_attr_local(&element, b"colId").and_then(|value| value.parse::<u32>().ok());
            }
            Ok(Event::Empty(element))
                if current_filter_column.is_some()
                    && xml_local_name(element.name().as_ref()) == b"filter" =>
            {
                let Some(value) = xml_attr_local(&element, b"val") else {
                    buffer.clear();
                    continue;
                };
                if let (Some(filter), Some(relative_column)) =
                    (metadata.auto_filter.as_mut(), current_filter_column)
                {
                    let column = filter.start_col.saturating_add(relative_column);
                    filter.column_filters.entry(column).or_default().push(value);
                }
            }
            Ok(Event::End(element))
                if xml_local_name(element.name().as_ref()) == b"filterColumn" =>
            {
                current_filter_column = None;
            }
            Ok(Event::Start(element)) if xml_local_name(element.name().as_ref()) == b"scenario" => {
                if current_scenario.is_none() {
                    let index = metadata.scenarios.len().saturating_add(1);
                    let name = xml_attr_local(&element, b"name")
                        .filter(|value| !value.trim().is_empty())
                        .unwrap_or_else(|| format!("Scenario {index}"));
                    current_scenario = Some(ScenarioModel {
                        id: format!("xlsx-scenario-{index}"),
                        name,
                        changes: Vec::new(),
                    });
                }
            }
            Ok(Event::Empty(element)) if xml_local_name(element.name().as_ref()) == b"scenario" => {
                if metadata.scenarios.len() < MAX_IMPORTED_SCENARIOS {
                    let index = metadata.scenarios.len().saturating_add(1);
                    let name = xml_attr_local(&element, b"name")
                        .filter(|value| !value.trim().is_empty())
                        .unwrap_or_else(|| format!("Scenario {index}"));
                    metadata.scenarios.push(ScenarioModel {
                        id: format!("xlsx-scenario-{index}"),
                        name,
                        changes: Vec::new(),
                    });
                }
            }
            Ok(Event::Empty(element))
                if current_scenario.is_some()
                    && xml_local_name(element.name().as_ref()) == b"inputCells" =>
            {
                let Some(scenario) = current_scenario.as_mut() else {
                    buffer.clear();
                    continue;
                };
                if scenario.changes.len() >= MAX_IMPORTED_SCENARIO_CELLS {
                    buffer.clear();
                    continue;
                }
                let Some((row, col)) = xml_attr_local(&element, b"r")
                    .and_then(|reference| parse_excel_cell_reference(&reference))
                else {
                    buffer.clear();
                    continue;
                };
                scenario
                    .changes
                    .push(redoc_sheet_engine::ScenarioCellChange {
                        row,
                        col,
                        raw_value: xml_attr_local(&element, b"val").unwrap_or_default(),
                    });
            }
            Ok(Event::End(element)) if xml_local_name(element.name().as_ref()) == b"scenario" => {
                if let Some(scenario) = current_scenario.take() {
                    if metadata.scenarios.len() < MAX_IMPORTED_SCENARIOS {
                        metadata.scenarios.push(scenario);
                    }
                }
            }
            Ok(Event::Start(element))
                if xml_local_name(element.name().as_ref()) == b"dataValidation" =>
            {
                current_validation = Some(ImportedValidation {
                    validation_type: xml_attr_local(&element, b"type").unwrap_or_default(),
                    sqref: xml_attr_local(&element, b"sqref").unwrap_or_default(),
                    formula: String::new(),
                });
            }
            Ok(Event::Empty(element))
                if xml_local_name(element.name().as_ref()) == b"dataValidation" =>
            {
                // A validation without a formula has no editable list to restore.
                current_validation = None;
            }
            Ok(Event::Start(element))
                if current_validation.is_some()
                    && xml_local_name(element.name().as_ref()) == b"formula1" =>
            {
                capturing_validation_formula = true;
                validation_formula_text.clear();
            }
            Ok(Event::Text(text)) if capturing_validation_formula => {
                if let Ok(value) = text.unescape() {
                    validation_formula_text.push_str(&value);
                }
            }
            Ok(Event::End(element))
                if capturing_validation_formula
                    && xml_local_name(element.name().as_ref()) == b"formula1" =>
            {
                if let Some(validation) = current_validation.as_mut() {
                    validation.formula = validation_formula_text.trim().to_string();
                }
                capturing_validation_formula = false;
                validation_formula_text.clear();
            }
            Ok(Event::End(element))
                if xml_local_name(element.name().as_ref()) == b"dataValidation" =>
            {
                if let Some(validation) = current_validation.take() {
                    if validation.validation_type == "list" {
                        let options = parse_list_validation_options(&validation.formula);
                        let formula = if options.is_none() {
                            normalize_list_validation_formula(&validation.formula)
                        } else {
                            None
                        };
                        if options.is_some() || formula.is_some() {
                            for token in validation.sqref.split_whitespace() {
                                let Some(range) = parse_excel_range(token) else {
                                    continue;
                                };
                                let row_count = u64::from(
                                    range
                                        .end_row
                                        .saturating_sub(range.start_row)
                                        .saturating_add(1),
                                );
                                let col_count = u64::from(
                                    range
                                        .end_col
                                        .saturating_sub(range.start_col)
                                        .saturating_add(1),
                                );
                                if row_count.saturating_mul(col_count)
                                    > MAX_IMPORTED_VALIDATION_CELLS
                                {
                                    continue;
                                }
                                for row in range.start_row..=range.end_row {
                                    for column in range.start_col..=range.end_col {
                                        metadata.validations.insert(
                                            format!("{row}:{column}"),
                                            ListValidation {
                                                validation_type: "list".to_string(),
                                                options: options.clone().unwrap_or_default(),
                                                formula: formula.clone(),
                                            },
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }
            Ok(Event::Empty(element))
                if xml_local_name(element.name().as_ref()) == b"mergeCell" =>
            {
                if let Some(range) =
                    xml_attr_local(&element, b"ref").and_then(|value| parse_excel_range(&value))
                {
                    metadata.merges.push(MergeRange {
                        start_row: range.start_row,
                        end_row: range.end_row,
                        start_col: range.start_col,
                        end_col: range.end_col,
                    });
                }
            }
            Ok(Event::Empty(element)) | Ok(Event::Start(element))
                if xml_local_name(element.name().as_ref()) == b"hyperlink" =>
            {
                let Some(reference) = xml_attr_local(&element, b"ref") else {
                    buffer.clear();
                    continue;
                };
                let target = xml_attr_local(&element, b"location")
                    .map(|location| format!("internal:{location}"))
                    .or_else(|| {
                        xml_attr_local(&element, b"id")
                            .and_then(|id| sheet_relationships.get(&id).cloned())
                    });
                let Some(target) = target.filter(|target| !target.trim().is_empty()) else {
                    buffer.clear();
                    continue;
                };
                for token in reference.split_whitespace() {
                    let Some(range) = parse_excel_range(token) else {
                        continue;
                    };
                    let row_count = u64::from(
                        range
                            .end_row
                            .saturating_sub(range.start_row)
                            .saturating_add(1),
                    );
                    let col_count = u64::from(
                        range
                            .end_col
                            .saturating_sub(range.start_col)
                            .saturating_add(1),
                    );
                    if row_count.saturating_mul(col_count) > MAX_IMPORTED_HYPERLINK_CELLS {
                        continue;
                    }
                    for row in range.start_row..=range.end_row {
                        for column in range.start_col..=range.end_col {
                            metadata
                                .hyperlinks
                                .insert(format!("{row}:{column}"), target.clone());
                        }
                    }
                }
            }
            Ok(Event::Empty(element)) | Ok(Event::Start(element))
                if xml_local_name(element.name().as_ref()) == b"tablePart" =>
            {
                if table_part_ids.len() < MAX_IMPORTED_TABLES {
                    if let Some(id) = xml_attr_local(&element, b"id") {
                        table_part_ids.push(id);
                    }
                }
            }
            Ok(Event::Start(element))
                if xml_local_name(element.name().as_ref()) == b"conditionalFormatting" =>
            {
                conditional_range =
                    xml_attr_local(&element, b"sqref").and_then(|value| parse_excel_range(&value));
            }
            Ok(Event::Start(element)) if xml_local_name(element.name().as_ref()) == b"cfRule" => {
                current_conditional = Some(ImportedConditionalRule {
                    range: conditional_range,
                    rule_type: xml_attr_local(&element, b"type").unwrap_or_default(),
                    operator: xml_attr_local(&element, b"operator"),
                    dxf_id: xml_attr_local(&element, b"dxfId")
                        .and_then(|value| value.parse::<usize>().ok()),
                    formula: xml_attr_local(&element, b"text"),
                    scale_colors: Vec::new(),
                    style: None,
                });
            }
            Ok(Event::Start(element))
                if current_conditional.is_some()
                    && xml_local_name(element.name().as_ref()) == b"formula" =>
            {
                capturing_formula = true;
                formula_text.clear();
            }
            Ok(Event::Text(text)) if capturing_formula => {
                if let Ok(value) = text.unescape() {
                    formula_text.push_str(&value);
                }
            }
            Ok(Event::End(element))
                if capturing_formula && xml_local_name(element.name().as_ref()) == b"formula" =>
            {
                if let Some(rule) = current_conditional.as_mut() {
                    if rule.rule_type != "containsText" || rule.formula.is_none() {
                        rule.formula = Some(formula_text.trim().to_string());
                    }
                }
                capturing_formula = false;
                formula_text.clear();
            }
            Ok(Event::Empty(element))
                if current_conditional.is_some()
                    && xml_local_name(element.name().as_ref()) == b"color" =>
            {
                if let Some(color) = xml_attr_local(&element, b"rgb") {
                    if let Some(rule) = current_conditional.as_mut() {
                        rule.scale_colors.push(color);
                    }
                }
            }
            Ok(Event::End(element)) if xml_local_name(element.name().as_ref()) == b"cfRule" => {
                if let Some(mut rule) = current_conditional.take() {
                    rule.style = rule.dxf_id.and_then(|id| dxf_styles.get(id).cloned());
                    if let Some(rule) = imported_conditional_rule(rule) {
                        metadata.conditional_formatting.push(rule);
                    }
                }
            }
            Ok(Event::End(element))
                if xml_local_name(element.name().as_ref()) == b"conditionalFormatting" =>
            {
                conditional_range = None;
            }
            Ok(Event::Empty(element)) | Ok(Event::Start(element)) => {
                match element.name().as_ref() {
                    b"c" => {
                        if let (Some(reference), Some(style_id)) = (
                            xml_attr(&element, b"r"),
                            xml_attr(&element, b"s").and_then(|value| value.parse().ok()),
                        ) {
                            metadata.cell_style_ids.insert(reference, style_id);
                        }
                    }
                    b"col" => {
                        let start = xml_attr(&element, b"min")
                            .and_then(|value| value.parse::<u32>().ok())
                            .unwrap_or(1);
                        let end = xml_attr(&element, b"max")
                            .and_then(|value| value.parse::<u32>().ok())
                            .unwrap_or(start);
                        if let Some(width) =
                            xml_attr(&element, b"width").and_then(|value| value.parse::<f64>().ok())
                        {
                            for column in start..=end {
                                metadata.col_widths.insert(column, width);
                            }
                        }
                    }
                    b"row" => {
                        if let (Some(row), Some(height)) = (
                            xml_attr(&element, b"r").and_then(|value| value.parse().ok()),
                            xml_attr(&element, b"ht").and_then(|value| value.parse().ok()),
                        ) {
                            metadata.row_heights.insert(row, height);
                        }
                    }
                    b"pane" => {
                        metadata.freeze_cols = xml_attr(&element, b"xSplit")
                            .and_then(|value| value.parse().ok())
                            .unwrap_or_default();
                        metadata.freeze_rows = xml_attr(&element, b"ySplit")
                            .and_then(|value| value.parse().ok())
                            .unwrap_or_default();
                    }
                    _ => {}
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
        buffer.clear();
    }
    for relationship_id in table_part_ids {
        let Some(target) = sheet_relationships
            .get(&relationship_id)
            .and_then(|target| relationship_target_path(&path, target))
        else {
            continue;
        };
        let Some(xml) = read_xlsx_entry(archive, &target) else {
            continue;
        };
        if let Some(table) = parse_xlsx_table_part(&xml, &relationship_id) {
            metadata.tables.push(table);
        }
    }
    metadata
}

fn imported_style(metadata: &ImportedXlsxMetadata, reference: &str) -> Option<CellStyle> {
    let style_id = *metadata.cell_style_ids.get(reference)?;
    let style = metadata.styles.get(style_id)?;
    let format = match style.format_code.as_deref() {
        Some(code) if code.contains('%') => Some("percent".to_string()),
        Some(code) if code.contains('$') || code.contains('€') || code.contains('£') => {
            Some("currency".to_string())
        }
        Some(code) if code.contains('0') || code.contains('#') => Some("number".to_string()),
        _ => match style.num_format_id {
            9 | 10 => Some("percent".to_string()),
            1..=4 => Some("number".to_string()),
            44 => Some("currency".to_string()),
            _ => None,
        },
    };
    let align = style.alignment.clone();
    (format.is_some()
        || align.is_some()
        || style.bold.is_some()
        || style.italic.is_some()
        || style.underline.is_some()
        || style.font_color.is_some()
        || style.bg_color.is_some()
        || style.font_family.is_some()
        || style.font_size.is_some()
        || style.borders.is_some())
    .then_some(CellStyle {
        bold: style.bold,
        italic: style.italic,
        underline: style.underline,
        font_color: style.font_color.clone(),
        bg_color: style.bg_color.clone(),
        align,
        format,
        wrap: None,
        v_align: None,
        validation: None,
        hyperlink: None,
        image: None,
        font_family: style.font_family.clone(),
        font_size: style.font_size,
        decimals: style.decimals,
        borders: style.borders.clone(),
    })
}

fn excel_reference(row: u32, mut column: u32) -> String {
    let mut letters = String::new();
    while column > 0 {
        let remainder = (column - 1) % 26;
        letters.insert(
            0,
            char::from_u32(u32::from(b'A') + remainder).unwrap_or('A'),
        );
        column = (column - 1) / 26;
    }
    format!("{letters}{row}")
}

fn parse_excel_cell_reference(reference: &str) -> Option<(u32, u32)> {
    let reference = reference.trim().trim_start_matches('$');
    let split = reference
        .find(|character: char| character.is_ascii_digit())
        .unwrap_or(reference.len());
    if split == 0 || split == reference.len() {
        return None;
    }
    let mut column = 0u32;
    for character in reference[..split].chars() {
        if !character.is_ascii_alphabetic() {
            return None;
        }
        column = column
            .checked_mul(26)?
            .checked_add(character.to_ascii_uppercase() as u32 - 'A' as u32 + 1)?;
    }
    let row = reference[split..].parse::<u32>().ok()?;
    (row > 0 && column > 0).then_some((row, column))
}

fn parse_excel_range(reference: &str) -> Option<redoc_sheet_engine::CellRange> {
    let first = reference.split_whitespace().next()?;
    let (start, end) = first.split_once(':').unwrap_or((first, first));
    let (start_row, start_col) = parse_excel_cell_reference(start)?;
    let (end_row, end_col) = parse_excel_cell_reference(end)?;
    (end_row >= start_row && end_col >= start_col).then_some(redoc_sheet_engine::CellRange {
        start_row,
        end_row,
        start_col,
        end_col,
    })
}

const MAX_HYPERLINK_TARGET_CHARS: usize = 2_048;

fn safe_xlsx_hyperlink_target(value: &str) -> Option<&str> {
    let target = value.trim();
    if target.is_empty()
        || target.chars().count() > MAX_HYPERLINK_TARGET_CHARS
        || target.chars().any(char::is_control)
    {
        return None;
    }
    if target
        .get(..9)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("internal:"))
    {
        let location = &target[9..];
        let valid = if let Some((sheet, cell)) = location.rsplit_once('!') {
            let sheet = sheet.trim();
            let quoted = sheet.starts_with('\'') && sheet.ends_with('\'');
            !sheet.is_empty()
                && (!sheet.contains('\'') || quoted)
                && !sheet
                    .chars()
                    .any(|character| matches!(character, '<' | '>' | '&' | '"'))
                && parse_excel_range(cell).is_some()
        } else {
            !location.is_empty()
                && (parse_excel_range(location).is_some()
                    || location.chars().enumerate().all(|(index, character)| {
                        (index == 0 && (character.is_ascii_alphabetic() || character == '_'))
                            || (index > 0
                                && (character.is_ascii_alphanumeric()
                                    || character == '_'
                                    || character == '.'))
                    }))
        };
        return valid.then_some(target);
    }
    if target
        .get(..7)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("http://"))
        || target
            .get(..8)
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case("https://"))
    {
        let scheme_end = target.find("://").unwrap_or(target.len());
        let authority = target[scheme_end + 3..]
            .split(['/', '?', '#'])
            .next()
            .unwrap_or_default();
        return (!authority.is_empty()
            && !authority
                .chars()
                .any(|character| matches!(character, ' ' | '\\' | '<' | '>' | '"')))
        .then_some(target);
    }
    if target
        .get(..7)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("mailto:"))
    {
        return target[7..]
            .split_once('@')
            .filter(|(local, domain)| {
                !local.is_empty() && !domain.is_empty() && !domain.contains(' ')
            })
            .map(|_| target);
    }
    if target
        .get(..4)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("tel:"))
    {
        return target[4..]
            .chars()
            .any(|character| character.is_ascii_digit())
            .then_some(target);
    }
    None
}

#[derive(Default)]
struct ImportedConditionalRule {
    range: Option<redoc_sheet_engine::CellRange>,
    rule_type: String,
    operator: Option<String>,
    dxf_id: Option<usize>,
    formula: Option<String>,
    scale_colors: Vec<String>,
    style: Option<ConditionalFormattingStyle>,
}

#[derive(Default)]
struct ImportedValidation {
    validation_type: String,
    sqref: String,
    formula: String,
}

const MAX_IMPORTED_VALIDATION_CELLS: u64 = 100_000;
const MAX_IMPORTED_HYPERLINK_CELLS: u64 = 100_000;
const MAX_VALIDATION_FORMULA_LEN: usize = 16_384;

fn parse_list_validation_options(formula: &str) -> Option<Vec<String>> {
    let value = formula.trim();
    let value = value.strip_prefix('"')?.strip_suffix('"')?;
    let options = value
        .split(',')
        .map(|option| option.replace("\"\"", "\""))
        .filter(|option| !option.is_empty())
        .collect::<Vec<_>>();
    (!options.is_empty()).then_some(options)
}

fn normalize_list_validation_formula(formula: &str) -> Option<String> {
    let trimmed = formula.trim();
    let value = trimmed.strip_prefix('=').unwrap_or(trimmed).trim();
    if value.is_empty()
        || value.len() > MAX_VALIDATION_FORMULA_LEN
        || value.chars().any(char::is_control)
    {
        return None;
    }
    Some(value.to_string())
}

fn imported_conditional_rule(rule: ImportedConditionalRule) -> Option<ConditionalFormattingRule> {
    let range = rule.range?;
    let rule_type = match rule.rule_type.as_str() {
        "containsText" => "textContains",
        "dataBar" => "dataBar",
        "colorScale" => "colorScale",
        "cellIs" => match rule.operator.as_deref() {
            Some("greaterThan") => "greaterThan",
            Some("lessThan") => "lessThan",
            Some("equal") => "equalTo",
            _ => return None,
        },
        _ => return None,
    };
    Some(ConditionalFormattingRule {
        range: redoc_sheet_engine::ConditionalFormattingRange {
            start_row: range.start_row,
            end_row: range.end_row,
            start_col: range.start_col,
            end_col: range.end_col,
        },
        rule_type: rule_type.to_string(),
        value: rule.formula,
        value2: None,
        style: rule.style,
        scale_colors: rule.scale_colors,
    })
}

fn color(value: &str) -> Option<Color> {
    let value = value.trim().trim_start_matches('#');
    (value.len() == 6)
        .then(|| u32::from_str_radix(value, 16).ok())
        .flatten()
        .map(Color::RGB)
}

/// Map the bounded Redoc border style vocabulary to native XLSX border kinds.
/// Unknown styles map to thin (the editor never produces them; imported files
/// are normalized on read).
fn border_kind(style: &str) -> Option<FormatBorder> {
    match style {
        "thin" => Some(FormatBorder::Thin),
        "medium" => Some(FormatBorder::Medium),
        "thick" => Some(FormatBorder::Thick),
        "dashed" => Some(FormatBorder::Dashed),
        "dotted" => Some(FormatBorder::Dotted),
        "double" => Some(FormatBorder::Double),
        _ => None,
    }
}

fn format_for_style(style: Option<&CellStyle>) -> Format {
    let mut format = Format::new();
    let Some(style) = style else {
        return format;
    };
    if style.bold == Some(true) {
        format = format.set_bold();
    }
    if style.italic == Some(true) {
        format = format.set_italic();
    }
    if style.underline == Some(true) {
        format = format.set_underline(FormatUnderline::Single);
    }
    if let Some(font_color) = style.font_color.as_deref().and_then(color) {
        format = format.set_font_color(font_color);
    }
    if let Some(bg_color) = style.bg_color.as_deref().and_then(color) {
        format = format.set_background_color(bg_color);
    }
    if let Some(borders) = style.borders.as_ref() {
        let edge = |edge: Option<&redoc_sheet_engine::BorderEdge>| {
            edge.and_then(|edge| {
                let border = border_kind(&edge.style)?;
                let color = edge.color.as_deref().and_then(color);
                Some((border, color))
            })
        };
        if let Some((kind, border_color)) = edge(borders.top.as_ref()) {
            format = match border_color {
                Some(c) => format.set_border_top(kind).set_border_top_color(c),
                None => format.set_border_top(kind),
            };
        }
        if let Some((kind, border_color)) = edge(borders.bottom.as_ref()) {
            format = match border_color {
                Some(c) => format.set_border_bottom(kind).set_border_bottom_color(c),
                None => format.set_border_bottom(kind),
            };
        }
        if let Some((kind, border_color)) = edge(borders.left.as_ref()) {
            format = match border_color {
                Some(c) => format.set_border_left(kind).set_border_left_color(c),
                None => format.set_border_left(kind),
            };
        }
        if let Some((kind, border_color)) = edge(borders.right.as_ref()) {
            format = match border_color {
                Some(c) => format.set_border_right(kind).set_border_right_color(c),
                None => format.set_border_right(kind),
            };
        }
    }
    if let Some(align) = style.align.as_deref() {
        format = format.set_align(match align {
            "center" => FormatAlign::Center,
            "right" => FormatAlign::Right,
            _ => FormatAlign::Left,
        });
    }
    if let Some(v_align) = style.v_align.as_deref() {
        format = format.set_align(match v_align {
            "top" => FormatAlign::Top,
            "bottom" => FormatAlign::Bottom,
            _ => FormatAlign::VerticalCenter,
        });
    }
    if style.wrap == Some(true) {
        format = format.set_text_wrap();
    }
    if let Some(family) = style.font_family.as_deref() {
        format = format.set_font_name(family);
    }
    if let Some(size) = style.font_size {
        if (1.0..=400.0).contains(&size) {
            format = format.set_font_size(size);
        }
    }
    if let Some(number_format) = style.format.as_deref() {
        // decimals takes precedence for numeric formats when present.
        let decimals = style.decimals.unwrap_or(2).min(10) as usize;
        let number_format = match number_format {
            "currency" => {
                if style.decimals.is_some() {
                    format!("${}", decimal_pattern("#,##0", decimals))
                } else {
                    "$#,##0.00".to_string()
                }
            }
            "percent" => {
                if style.decimals.is_some() {
                    format!("{}%", decimal_pattern("0", decimals))
                } else {
                    "0.00%".to_string()
                }
            }
            "number" => {
                if style.decimals.is_some() {
                    decimal_pattern("#,##0", decimals)
                } else {
                    "#,##0.00".to_string()
                }
            }
            _ => "General".to_string(),
        };
        format = format.set_num_format(number_format);
    }
    format
}

/// `"#,##0"` + `.{n}` decimal digits for the given count.
fn decimal_pattern(base: &str, decimals: usize) -> String {
    if decimals == 0 {
        base.to_string()
    } else {
        format!("{base}.{}", "0".repeat(decimals))
    }
}

fn chart_type_for(model: &ChartModel) -> ChartType {
    match model.chart_type.as_str() {
        "line" => ChartType::Line,
        "pie" => ChartType::Pie,
        "area" => ChartType::Area,
        "scatter" => ChartType::Scatter,
        "doughnut" => ChartType::Doughnut,
        _ => ChartType::Bar,
    }
}

fn format_for_conditional_style(style: Option<&ConditionalFormattingStyle>) -> Format {
    let mut format = Format::new();
    let Some(style) = style else {
        return format;
    };
    if style.bold == Some(true) {
        format = format.set_bold();
    }
    if style.italic == Some(true) {
        format = format.set_italic();
    }
    if let Some(font_color) = style.font_color.as_deref().and_then(color) {
        format = format.set_font_color(font_color);
    }
    if let Some(bg_color) = style.bg_color.as_deref().and_then(color) {
        format = format.set_background_color(bg_color);
    }
    format
}

fn insert_sheet_conditional_formats(
    worksheet: &mut rust_xlsxwriter::Worksheet,
    rules: &[ConditionalFormattingRule],
) -> Result<(), ExportError> {
    for rule in rules {
        let range = &rule.range;
        if range.start_row == 0
            || range.start_col == 0
            || range.end_row < range.start_row
            || range.end_col < range.start_col
            || range.end_col > u32::from(u16::MAX)
        {
            continue;
        }
        let first_row = range.start_row - 1;
        let last_row = range.end_row - 1;
        let first_col = (range.start_col - 1) as u16;
        let last_col = (range.end_col - 1) as u16;
        let style = format_for_conditional_style(rule.style.as_ref());

        match rule.rule_type.as_str() {
            "greaterThan" => {
                let Some(value) = rule
                    .value
                    .as_deref()
                    .and_then(|value| value.parse::<f64>().ok())
                else {
                    continue;
                };
                let conditional = ConditionalFormatCell::new()
                    .set_rule(ConditionalFormatCellRule::GreaterThan(value))
                    .set_format(style);
                worksheet
                    .add_conditional_format(first_row, first_col, last_row, last_col, &conditional)
                    .map_err(|error| ExportError::Xlsx(format!("{:?}", error)))?;
            }
            "lessThan" => {
                let Some(value) = rule
                    .value
                    .as_deref()
                    .and_then(|value| value.parse::<f64>().ok())
                else {
                    continue;
                };
                let conditional = ConditionalFormatCell::new()
                    .set_rule(ConditionalFormatCellRule::LessThan(value))
                    .set_format(style);
                worksheet
                    .add_conditional_format(first_row, first_col, last_row, last_col, &conditional)
                    .map_err(|error| ExportError::Xlsx(format!("{:?}", error)))?;
            }
            "equalTo" => {
                let Some(value) = rule.value.as_deref() else {
                    continue;
                };
                if let Ok(value) = value.parse::<f64>() {
                    let conditional = ConditionalFormatCell::new()
                        .set_rule(ConditionalFormatCellRule::EqualTo(value))
                        .set_format(style);
                    worksheet
                        .add_conditional_format(
                            first_row,
                            first_col,
                            last_row,
                            last_col,
                            &conditional,
                        )
                        .map_err(|error| ExportError::Xlsx(format!("{:?}", error)))?;
                } else {
                    let conditional = ConditionalFormatCell::new()
                        .set_rule(ConditionalFormatCellRule::EqualTo(value.to_string()))
                        .set_format(style);
                    worksheet
                        .add_conditional_format(
                            first_row,
                            first_col,
                            last_row,
                            last_col,
                            &conditional,
                        )
                        .map_err(|error| ExportError::Xlsx(format!("{:?}", error)))?;
                }
            }
            "textContains" => {
                let Some(value) = rule.value.as_deref() else {
                    continue;
                };
                let conditional = ConditionalFormatText::new()
                    .set_rule(ConditionalFormatTextRule::Contains(value.to_string()))
                    .set_format(style);
                worksheet
                    .add_conditional_format(first_row, first_col, last_row, last_col, &conditional)
                    .map_err(|error| ExportError::Xlsx(format!("{:?}", error)))?;
            }
            "dataBar" => {
                let conditional = if let Some(fill) = rule
                    .style
                    .as_ref()
                    .and_then(|style| style.bg_color.as_deref())
                    .and_then(color)
                {
                    ConditionalFormatDataBar::new().set_fill_color(fill)
                } else {
                    ConditionalFormatDataBar::new()
                };
                worksheet
                    .add_conditional_format(first_row, first_col, last_row, last_col, &conditional)
                    .map_err(|error| ExportError::Xlsx(format!("{:?}", error)))?;
            }
            "colorScale" => {
                let colors = rule
                    .scale_colors
                    .iter()
                    .filter_map(|value| color(value))
                    .collect::<Vec<_>>();
                if colors.len() >= 3 {
                    let conditional = ConditionalFormat3ColorScale::new()
                        .set_minimum_color(colors[0])
                        .set_midpoint_color(colors[1])
                        .set_maximum_color(colors[2]);
                    worksheet
                        .add_conditional_format(
                            first_row,
                            first_col,
                            last_row,
                            last_col,
                            &conditional,
                        )
                        .map_err(|error| ExportError::Xlsx(format!("{:?}", error)))?;
                } else {
                    let conditional = colors
                        .first()
                        .copied()
                        .map(|minimum| {
                            ConditionalFormat2ColorScale::new()
                                .set_minimum_color(minimum)
                                .set_maximum_color(
                                    colors.get(1).copied().unwrap_or(Color::RGB(0x63BE7B)),
                                )
                        })
                        .unwrap_or_else(ConditionalFormat2ColorScale::new);
                    worksheet
                        .add_conditional_format(
                            first_row,
                            first_col,
                            last_row,
                            last_col,
                            &conditional,
                        )
                        .map_err(|error| ExportError::Xlsx(format!("{:?}", error)))?;
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn insert_sheet_charts(
    worksheet: &mut rust_xlsxwriter::Worksheet,
    sheet_name: &str,
    charts: &[ChartModel],
) -> Result<(), ExportError> {
    for (offset, chart_model) in charts.iter().enumerate() {
        if chart_model.end_row < chart_model.start_row
            || chart_model.end_col < chart_model.start_col
        {
            continue;
        }
        let first_row = chart_model.start_row.saturating_sub(1);
        let last_row = chart_model.end_row.saturating_sub(1);
        let first_col = (chart_model.start_col.saturating_sub(1)) as u16;
        let last_col = (chart_model.end_col.saturating_sub(1)) as u16;
        let value_col = if first_col < last_col {
            first_col + 1
        } else {
            first_col
        };

        let mut chart = Chart::new(chart_type_for(chart_model));
        chart
            .add_series()
            .set_categories((sheet_name, first_row, first_col, last_row, first_col))
            .set_values((sheet_name, first_row, value_col, last_row, value_col));
        if let Some(title) = chart_model
            .title
            .as_deref()
            .filter(|title| !title.is_empty())
        {
            chart.title().set_name(title);
        } else {
            chart.title().set_name("Chart");
        }

        let insert_row = last_row.saturating_add(2 + offset as u32);
        let insert_col = first_col;
        worksheet
            .insert_chart(insert_row, insert_col, &chart)
            .map_err(|error| ExportError::Xlsx(format!("{:?}", error)))?;
    }
    Ok(())
}

fn insert_sheet_auto_filter(
    worksheet: &mut rust_xlsxwriter::Worksheet,
    auto_filter: Option<&AutoFilterState>,
) -> Result<(), ExportError> {
    let Some(filter) = auto_filter.filter(|filter| filter.enabled) else {
        return Ok(());
    };
    if filter.end_row < filter.start_row || filter.end_col < filter.start_col {
        return Ok(());
    }
    worksheet
        .autofilter(
            filter.start_row.saturating_sub(1),
            filter.start_col.saturating_sub(1) as u16,
            filter.end_row.saturating_sub(1),
            filter.end_col.saturating_sub(1) as u16,
        )
        .map_err(|error| ExportError::Xlsx(format!("{:?}", error)))?;
    for (column, values) in &filter.column_filters {
        if values.is_empty() || *column < filter.start_col || *column > filter.end_col {
            continue;
        }
        let condition = values
            .iter()
            .fold(FilterCondition::new(), |condition, value| {
                condition.add_list_filter(value.as_str())
            });
        worksheet
            .filter_column(column.saturating_sub(1) as u16, &condition)
            .map_err(|error| ExportError::Xlsx(format!("{:?}", error)))?;
    }
    Ok(())
}

fn table_style_for_name(name: Option<&str>) -> TableStyle {
    match name.unwrap_or_default() {
        "TableStyleLight1" => TableStyle::Light1,
        "TableStyleLight2" => TableStyle::Light2,
        "TableStyleLight9" => TableStyle::Light9,
        "TableStyleMedium2" => TableStyle::Medium2,
        "TableStyleMedium4" => TableStyle::Medium4,
        "TableStyleMedium9" => TableStyle::Medium9,
        "TableStyleMedium10" => TableStyle::Medium10,
        "TableStyleMedium16" => TableStyle::Medium16,
        "TableStyleDark1" => TableStyle::Dark1,
        "TableStyleDark2" => TableStyle::Dark2,
        _ => TableStyle::Medium9,
    }
}

fn insert_sheet_tables(
    worksheet: &mut rust_xlsxwriter::Worksheet,
    tables: &[redoc_sheet_engine::TableModel],
) -> Result<(), ExportError> {
    for table_model in tables {
        let range = &table_model.range;
        if table_model.name.trim().is_empty()
            || range.start_row == 0
            || range.start_col == 0
            || range.end_row < range.start_row
            || range.end_col < range.start_col
            || range.end_col > u32::from(u16::MAX)
        {
            return Err(ExportError::Xlsx(format!(
                "invalid table metadata for {:?}",
                table_model.name
            )));
        }
        let mut table = Table::new()
            .set_name(table_model.name.trim())
            .set_header_row(table_model.show_header_row)
            .set_total_row(table_model.show_total_row)
            .set_style(table_style_for_name(table_model.style.as_deref()));
        if !table_model.columns.is_empty() {
            let columns = table_model
                .columns
                .iter()
                .map(|name| TableColumn::new().set_header(name))
                .collect::<Vec<_>>();
            table = table.set_columns(&columns);
        }
        worksheet
            .add_table(
                range.start_row.saturating_sub(1),
                (range.start_col.saturating_sub(1)) as u16,
                range.end_row.saturating_sub(1),
                (range.end_col.saturating_sub(1)) as u16,
                &table,
            )
            .map_err(|error| ExportError::Xlsx(format!("{:?}", error)))?;
    }
    Ok(())
}

fn xlsx_defined_name_key(named_range: &NamedRange) -> String {
    named_range
        .sheet
        .as_deref()
        .map(|sheet| {
            let escaped = sheet.replace('\'', "''");
            let needs_quotes = sheet
                .chars()
                .any(|character| !character.is_ascii_alphanumeric() && character != '_');
            if needs_quotes {
                format!("'{escaped}'!{}", named_range.name)
            } else {
                format!("{escaped}!{}", named_range.name)
            }
        })
        .unwrap_or_else(|| named_range.name.clone())
}

fn xlsx_defined_name_formula(named_range: &NamedRange) -> String {
    let range = named_range.range_str.trim();
    if range.starts_with('=') {
        range.to_string()
    } else if range.contains('!') || named_range.sheet.is_none() {
        format!("={range}")
    } else {
        let escaped = named_range
            .sheet
            .as_deref()
            .unwrap_or_default()
            .replace('\'', "''");
        let needs_quotes = named_range.sheet.as_deref().is_some_and(|sheet| {
            sheet
                .chars()
                .any(|character| !character.is_ascii_alphanumeric() && character != '_')
        });
        let sheet = if needs_quotes {
            format!("'{escaped}'")
        } else {
            escaped
        };
        format!("={sheet}!{range}")
    }
}

const MAX_EXPORTED_SCENARIOS: usize = 256;
const MAX_EXPORTED_SCENARIO_CELLS: usize = 100_000;

fn xml_escape_attribute(value: &str) -> String {
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
            character
                if character == '\u{0}'
                    || (character < '\u{20}'
                        && character != '\u{9}'
                        && character != '\u{A}'
                        && character != '\u{D}') => {}
            character => escaped.push(character),
        }
    }
    escaped
}

fn scenarios_xml(scenarios: &[ScenarioModel]) -> Result<String, ExportError> {
    if scenarios.is_empty() {
        return Ok(String::new());
    }
    if scenarios.len() > MAX_EXPORTED_SCENARIOS {
        return Err(ExportError::Xlsx(format!(
            "workbook contains too many scenarios (maximum {MAX_EXPORTED_SCENARIOS})"
        )));
    }
    let mut references = Vec::new();
    let mut xml = String::from("<scenarios current=\"0\" show=\"0\"");
    for scenario in scenarios {
        if scenario.changes.len() > MAX_EXPORTED_SCENARIO_CELLS {
            return Err(ExportError::Xlsx(format!(
                "scenario {:?} contains too many input cells (maximum {MAX_EXPORTED_SCENARIO_CELLS})",
                scenario.name
            )));
        }
        for change in &scenario.changes {
            if change.row == 0 || change.col == 0 {
                return Err(ExportError::Xlsx(format!(
                    "scenario {:?} contains an invalid cell reference",
                    scenario.name
                )));
            }
            let reference = excel_reference(change.row, change.col);
            if !references.iter().any(|existing| existing == &reference) {
                references.push(reference);
            }
        }
    }
    if !references.is_empty() {
        xml.push_str(" sqref=\"");
        xml.push_str(&xml_escape_attribute(&references.join(" ")));
        xml.push('"');
    }
    xml.push('>');
    for scenario in scenarios {
        let name = scenario.name.trim();
        if name.is_empty() {
            return Err(ExportError::Xlsx(
                "scenario names must not be empty".to_string(),
            ));
        }
        xml.push_str(&format!(
            "<scenario name=\"{}\" locked=\"1\" hidden=\"0\" count=\"{}\" user=\"Redoc\">",
            xml_escape_attribute(name),
            scenario.changes.len()
        ));
        for change in &scenario.changes {
            xml.push_str(&format!(
                "<inputCells r=\"{}\" val=\"{}\"/>",
                excel_reference(change.row, change.col),
                xml_escape_attribute(&change.raw_value)
            ));
        }
        xml.push_str("</scenario>");
    }
    xml.push_str("</scenarios>");
    Ok(xml)
}

fn inject_sheet_scenarios(
    sheet_xml: &str,
    scenarios: &[ScenarioModel],
) -> Result<String, ExportError> {
    let scenario_xml = scenarios_xml(scenarios)?;
    if scenario_xml.is_empty() {
        return Ok(sheet_xml.to_string());
    }
    let insertion = if let Some(sheet_data_end) = sheet_xml.find("</sheetData>") {
        sheet_data_end + "</sheetData>".len()
    } else if let Some(sheet_data_start) = sheet_xml.find("<sheetData") {
        let Some(tag_end_offset) = sheet_xml[sheet_data_start..].find('>') else {
            return Err(ExportError::Xlsx(
                "worksheet has a malformed sheetData element".to_string(),
            ));
        };
        let tag_end = sheet_data_start + tag_end_offset;
        if !sheet_xml[sheet_data_start..=tag_end]
            .trim_end()
            .ends_with("/>")
        {
            return Err(ExportError::Xlsx(
                "worksheet has a malformed sheetData element".to_string(),
            ));
        }
        tag_end + 1
    } else {
        return Err(ExportError::Xlsx(
            "worksheet is missing required sheetData element".to_string(),
        ));
    };
    let mut output = String::with_capacity(sheet_xml.len() + scenario_xml.len());
    output.push_str(&sheet_xml[..insertion]);
    output.push_str(&scenario_xml);
    output.push_str(&sheet_xml[insertion..]);
    Ok(output)
}

fn rewrite_xlsx_with_scenarios(
    bytes: Vec<u8>,
    workbook: &WorkbookModel,
) -> Result<Vec<u8>, ExportError> {
    if workbook
        .sheets
        .iter()
        .all(|sheet| sheet.scenarios.is_empty())
    {
        return Ok(bytes);
    }
    let mut input = zip::ZipArchive::new(Cursor::new(bytes))
        .map_err(|error| ExportError::Xlsx(format!("could not reopen XLSX package: {error:?}")))?;
    let mut output = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for index in 0..input.len() {
        let mut entry = input.by_index(index).map_err(|error| {
            ExportError::Xlsx(format!("could not read XLSX package entry: {error:?}"))
        })?;
        let name = entry.name().to_string();
        let compression = entry.compression();
        let modified = entry.last_modified();
        let modified = modified.unwrap_or_default();
        let permissions = entry.unix_mode();
        let directory = entry.is_dir();
        let mut data = Vec::new();
        entry.read_to_end(&mut data).map_err(|error| {
            ExportError::Xlsx(format!("could not read XLSX package entry: {error:?}"))
        })?;
        drop(entry);

        let sheet_index = name
            .strip_prefix("xl/worksheets/sheet")
            .and_then(|suffix| suffix.strip_suffix(".xml"))
            .and_then(|value| value.parse::<usize>().ok())
            .and_then(|value| value.checked_sub(1));
        if let Some(sheet_index) = sheet_index {
            if let Some(scenarios) = workbook
                .sheets
                .get(sheet_index)
                .map(|sheet| sheet.scenarios.as_slice())
                .filter(|scenarios| !scenarios.is_empty())
            {
                let xml = String::from_utf8(data).map_err(|_| {
                    ExportError::Xlsx(format!("worksheet package entry {name} is not UTF-8 XML"))
                })?;
                data = inject_sheet_scenarios(&xml, scenarios)?.into_bytes();
            }
        }

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
            output.write_all(&data).map_err(|error| {
                ExportError::Xlsx(format!("could not write XLSX entry: {error:?}"))
            })?;
        }
    }
    output
        .finish()
        .map(|cursor| cursor.into_inner())
        .map_err(|error| ExportError::Xlsx(format!("could not finish XLSX package: {error:?}")))
}

pub fn export_workbook_to_xlsx(workbook: &WorkbookModel) -> Result<Vec<u8>, ExportError> {
    let mut wb = Workbook::new();

    for sheet_data in &workbook.sheets {
        let ws = wb.add_worksheet();
        let _ = ws.set_name(&sheet_data.name);

        for (column, width) in &sheet_data.col_widths {
            let _ = ws.set_column_width(column.saturating_sub(1) as u16, *width);
        }
        for (row, height) in &sheet_data.row_heights {
            let _ = ws.set_row_height(row.saturating_sub(1), *height);
        }
        if sheet_data.freeze_rows > 0 || sheet_data.freeze_cols > 0 {
            let _ = ws.set_freeze_panes(sheet_data.freeze_rows, sheet_data.freeze_cols as u16);
        }

        for merge in &sheet_data.merges {
            let _ = ws.merge_range(
                merge.start_row.saturating_sub(1),
                (merge.start_col.saturating_sub(1)) as u16,
                merge.end_row.saturating_sub(1),
                (merge.end_col.saturating_sub(1)) as u16,
                "",
                &Format::new(),
            );
        }

        for (key, cell) in &sheet_data.cells {
            if let Ok((r, c)) = redoc_sheet_engine::parse_key(key) {
                let row = r.saturating_sub(1);
                let col = (c.saturating_sub(1)) as u16;
                let format = format_for_style(cell.style.as_ref());

                if let Some(href) = cell
                    .style
                    .as_ref()
                    .and_then(|style| style.hyperlink.as_deref())
                    .filter(|href| !href.trim().is_empty())
                {
                    let href = safe_xlsx_hyperlink_target(href).ok_or_else(|| {
                        ExportError::Xlsx("unsupported or malformed hyperlink target".to_string())
                    })?;
                    ws.write_url_with_options(
                        row,
                        col,
                        href,
                        cell.raw_value.clone(),
                        String::new(),
                        Some(&format),
                    )
                    .map_err(|error| ExportError::Xlsx(format!("{:?}", error)))?;
                } else if let Some(ref f) = cell.formula {
                    let _ = ws.write_formula_with_format(row, col, f.as_str(), &format);
                } else if let Ok(n) = cell.raw_value.parse::<f64>() {
                    let _ = ws.write_number_with_format(row, col, n, &format);
                } else {
                    let _ = ws.write_string_with_format(row, col, &cell.raw_value, &format);
                }

                if let Some(validation) = cell
                    .style
                    .as_ref()
                    .and_then(|style| style.validation.as_ref())
                    .filter(|validation| validation.validation_type == "list")
                {
                    let rule = if let Some(formula) = validation
                        .formula
                        .as_deref()
                        .and_then(normalize_list_validation_formula)
                    {
                        Some(DataValidation::new().allow_list_formula(formula.as_str().into()))
                    } else if !validation.options.is_empty() {
                        Some(
                            DataValidation::new()
                                .allow_list_strings(validation.options.as_slice())
                                .map_err(|error| ExportError::Xlsx(format!("{:?}", error)))?,
                        )
                    } else {
                        None
                    };
                    if let Some(rule) = rule {
                        ws.add_data_validation(row, col, row, col, &rule)
                            .map_err(|error| ExportError::Xlsx(format!("{:?}", error)))?;
                    }
                }

                // Native XLSX drawings support PNG and JPEG buffers. Other image
                // formats stay in the canonical Redoc style and are reported by
                // the compatibility inspector rather than failing an export.
                if let Some(source) = cell.style.as_ref().and_then(|style| style.image.as_deref()) {
                    if let Some(bytes) = xlsx_image_bytes(source) {
                        if let Ok(image) = Image::new_from_buffer(&bytes) {
                            ws.insert_image_fit_to_cell(row, col, &image, true)
                                .map_err(|error| ExportError::Xlsx(format!("{:?}", error)))?;
                        }
                    }
                }
            }
        }

        insert_sheet_auto_filter(ws, sheet_data.auto_filter.as_ref())?;
        insert_sheet_tables(ws, &sheet_data.tables)?;
        insert_sheet_conditional_formats(ws, &sheet_data.conditional_formatting)?;
        insert_sheet_charts(ws, &sheet_data.name, &sheet_data.charts)?;
    }

    for named_range in &workbook.named_ranges {
        let formula = xlsx_defined_name_formula(named_range);
        wb.define_name(xlsx_defined_name_key(named_range), &formula)
            .map_err(|error| ExportError::Xlsx(format!("{:?}", error)))?;
    }

    let buf = wb
        .save_to_buffer()
        .map_err(|e| ExportError::Xlsx(format!("{:?}", e)))?;
    let buf = rewrite_xlsx_with_scenarios(buf, workbook)?;
    crate::xlsx_pivot::rewrite_xlsx_with_pivots(buf, workbook)
}

fn archive_has_charts(archive: &mut zip::ZipArchive<std::io::Cursor<Vec<u8>>>) -> bool {
    for index in 0..archive.len() {
        if let Ok(file) = archive.by_index(index) {
            if file.name().starts_with("xl/charts/") {
                return true;
            }
        }
    }
    false
}

const MAX_XLSX_XML_BYTES: u64 = 16 * 1024 * 1024;
const MAX_XLSX_FILE_BYTES: u64 = 512 * 1024 * 1024;
const MAX_XLSX_ARCHIVE_ENTRIES: usize = 10_000;
const MAX_IMPORTED_TABLES: usize = 1_000;
const MAX_IMPORTED_SCENARIOS: usize = 256;
const MAX_IMPORTED_SCENARIO_CELLS: usize = 100_000;

fn xml_local_name(name: &[u8]) -> &[u8] {
    name.rsplit(|byte| *byte == b':').next().unwrap_or(name)
}

fn xml_attr_local(element: &BytesStart<'_>, name: &[u8]) -> Option<String> {
    element.attributes().flatten().find_map(|attribute| {
        if xml_local_name(attribute.key.as_ref()) != name {
            return None;
        }
        let value = String::from_utf8_lossy(attribute.value.as_ref()).into_owned();
        Some(
            quick_xml::escape::unescape(&value)
                .map(|value| value.into_owned())
                .unwrap_or(value),
        )
    })
}

fn read_xlsx_entry(
    archive: &mut zip::ZipArchive<std::io::Cursor<Vec<u8>>>,
    path: &str,
) -> Option<Vec<u8>> {
    let mut file = archive.by_name(path).ok()?;
    if file.size() > MAX_XLSX_XML_BYTES {
        return None;
    }
    let mut xml = Vec::new();
    file.read_to_end(&mut xml).ok()?;
    Some(xml)
}

fn normalize_xlsx_target(base_dir: &str, target: &str) -> Option<String> {
    if target.contains("://") {
        return None;
    }
    let mut parts = if target.starts_with('/') {
        Vec::new()
    } else {
        base_dir
            .split('/')
            .filter(|part| !part.is_empty())
            .map(str::to_string)
            .collect::<Vec<_>>()
    };
    for part in target.trim_start_matches('/').split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            value => parts.push(value.to_string()),
        }
    }
    Some(parts.join("/"))
}

fn relationship_part_path(part_path: &str) -> Option<String> {
    let (directory, file_name) = part_path.rsplit_once('/')?;
    Some(format!("{directory}/_rels/{file_name}.rels"))
}

fn relationship_target_path(part_path: &str, target: &str) -> Option<String> {
    let base_dir = part_path
        .rsplit_once('/')
        .map(|(directory, _)| directory)
        .unwrap_or_default();
    normalize_xlsx_target(base_dir, target)
}

fn parse_relationships(xml: &[u8]) -> HashMap<String, String> {
    let mut reader = XmlReader::from_reader(xml);
    reader.config_mut().trim_text(true);
    let mut buffer = Vec::new();
    let mut relationships = HashMap::new();
    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Empty(element)) | Ok(Event::Start(element))
                if xml_local_name(element.name().as_ref()) == b"Relationship" =>
            {
                if let (Some(id), Some(target)) = (
                    xml_attr_local(&element, b"Id"),
                    xml_attr_local(&element, b"Target"),
                ) {
                    relationships.insert(id, target);
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
        buffer.clear();
    }
    relationships
}

fn parse_xlsx_table_part(xml: &[u8], fallback_id: &str) -> Option<redoc_sheet_engine::TableModel> {
    let mut reader = XmlReader::from_reader(xml);
    reader.config_mut().trim_text(true);
    let mut buffer = Vec::new();
    let mut table = None;
    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(element)) if xml_local_name(element.name().as_ref()) == b"table" => {
                let name = xml_attr_local(&element, b"displayName")
                    .or_else(|| xml_attr_local(&element, b"name"))
                    .unwrap_or_default();
                let range =
                    xml_attr_local(&element, b"ref").and_then(|value| parse_excel_range(&value));
                if !name.is_empty() {
                    if let Some(range) = range {
                        table = Some(redoc_sheet_engine::TableModel {
                            id: xml_attr_local(&element, b"id")
                                .unwrap_or_else(|| fallback_id.to_string()),
                            name,
                            range,
                            columns: Vec::new(),
                            style: None,
                            show_header_row: xml_attr_local(&element, b"headerRowCount")
                                .and_then(|value| value.parse::<u32>().ok())
                                .map(|count| count > 0)
                                .unwrap_or(true),
                            show_total_row: xml_attr_local(&element, b"totalsRowCount")
                                .and_then(|value| value.parse::<u32>().ok())
                                .map(|count| count > 0)
                                .unwrap_or(false),
                        });
                    }
                }
            }
            Ok(Event::Empty(element)) | Ok(Event::Start(element))
                if xml_local_name(element.name().as_ref()) == b"tableColumn" =>
            {
                if let (Some(table), Some(name)) = (
                    table.as_mut(),
                    xml_attr_local(&element, b"name").filter(|name| !name.is_empty()),
                ) {
                    if table.columns.len() < 16_384 {
                        table.columns.push(name);
                    }
                }
            }
            Ok(Event::Empty(element))
                if xml_local_name(element.name().as_ref()) == b"tableStyleInfo" =>
            {
                if let Some(table) = table.as_mut() {
                    table.style = xml_attr_local(&element, b"name");
                }
            }
            Ok(Event::End(element)) if xml_local_name(element.name().as_ref()) == b"table" => {
                return table;
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
        buffer.clear();
    }
    table
}

fn parse_xlsx_named_ranges(
    archive: &mut zip::ZipArchive<std::io::Cursor<Vec<u8>>>,
    sheet_names: &[String],
) -> Vec<NamedRange> {
    let Some(xml) = read_xlsx_entry(archive, "xl/workbook.xml") else {
        return Vec::new();
    };
    let mut reader = XmlReader::from_reader(xml.as_slice());
    reader.config_mut().trim_text(true);
    let mut buffer = Vec::new();
    let mut current: Option<(String, Option<usize>, String)> = None;
    let mut in_defined_name = false;
    let mut ranges = Vec::new();
    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(element))
                if xml_local_name(element.name().as_ref()) == b"definedName" =>
            {
                let name = xml_attr_local(&element, b"name").unwrap_or_default();
                let local_sheet = xml_attr_local(&element, b"localSheetId")
                    .and_then(|value| value.parse::<usize>().ok());
                current = Some((name, local_sheet, String::new()));
                in_defined_name = true;
            }
            Ok(Event::Text(text)) if in_defined_name => {
                if let Some((_, _, formula)) = current.as_mut() {
                    if let Ok(value) = text.unescape() {
                        formula.push_str(&value);
                    }
                }
            }
            Ok(Event::End(element))
                if xml_local_name(element.name().as_ref()) == b"definedName" =>
            {
                in_defined_name = false;
                if let Some((name, local_sheet, formula)) = current.take() {
                    if name.is_empty() || name.starts_with("_xlnm.") || formula.trim().is_empty() {
                        buffer.clear();
                        continue;
                    }
                    let formula = formula.trim().trim_start_matches('=').trim();
                    let sheet = local_sheet.and_then(|index| sheet_names.get(index).cloned());
                    let range_str = if local_sheet.is_some() {
                        formula
                            .split_once('!')
                            .map(|(_, value)| value)
                            .unwrap_or(formula)
                            .to_string()
                    } else {
                        formula.to_string()
                    };
                    ranges.push(NamedRange {
                        name,
                        range_str,
                        sheet,
                    });
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
        buffer.clear();
    }
    ranges
}

fn parse_sheet_drawing_id(xml: &[u8]) -> Option<String> {
    let mut reader = XmlReader::from_reader(xml);
    let mut buffer = Vec::new();
    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Empty(element)) | Ok(Event::Start(element))
                if xml_local_name(element.name().as_ref()) == b"drawing" =>
            {
                return xml_attr_local(&element, b"id");
            }
            Ok(Event::Eof) | Err(_) => return None,
            _ => {}
        }
        buffer.clear();
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum DrawingAnchorKind {
    #[default]
    Chart,
    Image,
}

#[derive(Default)]
struct DrawingAnchor {
    kind: DrawingAnchorKind,
    relationship: String,
    start_row: u32,
    end_row: u32,
    start_col: u32,
    end_col: u32,
}

#[derive(Clone, Copy)]
enum AnchorSection {
    From,
    To,
}

fn parse_drawing_anchors(xml: &[u8]) -> Vec<DrawingAnchor> {
    let mut reader = XmlReader::from_reader(xml);
    reader.config_mut().trim_text(true);
    let mut buffer = Vec::new();
    let mut anchor: Option<DrawingAnchor> = None;
    let mut section = None;
    let mut capture = None::<Vec<u8>>;
    let mut captured_text = String::new();
    let mut anchors = Vec::new();
    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(element)) => {
                let name = xml_local_name(element.name().as_ref()).to_vec();
                match name.as_slice() {
                    b"twoCellAnchor" | b"oneCellAnchor" => anchor = Some(DrawingAnchor::default()),
                    b"from" => section = Some(AnchorSection::From),
                    b"to" => section = Some(AnchorSection::To),
                    b"row" | b"col" if anchor.is_some() => {
                        capture = Some(name);
                        captured_text.clear();
                    }
                    b"chart" if anchor.is_some() => {
                        if let Some(anchor) = anchor.as_mut() {
                            anchor.kind = DrawingAnchorKind::Chart;
                            anchor.relationship =
                                xml_attr_local(&element, b"id").unwrap_or_default();
                        }
                    }
                    b"pic" if anchor.is_some() => {
                        if let Some(anchor) = anchor.as_mut() {
                            anchor.kind = DrawingAnchorKind::Image;
                        }
                    }
                    b"blip" if anchor.is_some() => {
                        if let Some(anchor) = anchor.as_mut() {
                            anchor.kind = DrawingAnchorKind::Image;
                            anchor.relationship =
                                xml_attr_local(&element, b"embed").unwrap_or_default();
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Empty(element)) => {
                let name = xml_local_name(element.name().as_ref()).to_vec();
                if name.as_slice() == b"chart" {
                    if let Some(anchor) = anchor.as_mut() {
                        anchor.kind = DrawingAnchorKind::Chart;
                        anchor.relationship = xml_attr_local(&element, b"id").unwrap_or_default();
                    }
                } else if name.as_slice() == b"blip" {
                    if let Some(anchor) = anchor.as_mut() {
                        anchor.kind = DrawingAnchorKind::Image;
                        anchor.relationship =
                            xml_attr_local(&element, b"embed").unwrap_or_default();
                    }
                }
            }
            Ok(Event::Text(text)) if capture.is_some() => {
                if let Ok(value) = text.unescape() {
                    captured_text.push_str(&value);
                }
            }
            Ok(Event::End(element)) => {
                let name = xml_local_name(element.name().as_ref()).to_vec();
                if matches!(name.as_slice(), b"row" | b"col")
                    && capture.as_deref() == Some(name.as_slice())
                {
                    if let (Some(anchor), Some(section)) = (anchor.as_mut(), section) {
                        let value = captured_text.parse::<u32>().unwrap_or_default();
                        match (section, name.as_slice()) {
                            (AnchorSection::From, b"row") => anchor.start_row = value,
                            (AnchorSection::From, b"col") => anchor.start_col = value,
                            (AnchorSection::To, b"row") => anchor.end_row = value,
                            (AnchorSection::To, b"col") => anchor.end_col = value,
                            _ => {}
                        }
                    }
                    capture = None;
                    captured_text.clear();
                } else if name.as_slice() == b"from" || name.as_slice() == b"to" {
                    section = None;
                } else if matches!(name.as_slice(), b"twoCellAnchor" | b"oneCellAnchor") {
                    if let Some(mut anchor) = anchor.take() {
                        if anchor.end_row == 0 && anchor.end_col == 0 {
                            anchor.end_row = anchor.start_row.saturating_add(5);
                            anchor.end_col = anchor.start_col.saturating_add(8);
                        }
                        if !anchor.relationship.is_empty() {
                            anchors.push(anchor);
                        }
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buffer.clear();
    }
    anchors
}

fn parse_chart_metadata(xml: &[u8]) -> (String, Option<String>) {
    const CHART_TYPES: &[(&str, &str)] = &[
        ("lineChart", "line"),
        ("pieChart", "pie"),
        ("doughnutChart", "doughnut"),
        ("scatterChart", "scatter"),
        ("areaChart", "area"),
        ("barChart", "bar"),
    ];
    let chart_type = CHART_TYPES
        .iter()
        .find(|(marker, _)| {
            xml.windows(marker.len())
                .any(|window| window == marker.as_bytes())
        })
        .map(|(_, name)| *name)
        .unwrap_or("bar")
        .to_string();
    let mut reader = XmlReader::from_reader(xml);
    reader.config_mut().trim_text(true);
    let mut buffer = Vec::new();
    let mut in_text = false;
    let mut title = None;
    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(element)) => in_text = xml_local_name(element.name().as_ref()) == b"t",
            Ok(Event::Text(text)) if in_text && title.is_none() => {
                title = text.unescape().ok().map(|value| value.into_owned());
            }
            Ok(Event::End(element)) if xml_local_name(element.name().as_ref()) == b"t" => {
                in_text = false;
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
        buffer.clear();
    }
    (chart_type, title.filter(|value| !value.trim().is_empty()))
}

fn import_sheet_charts(
    archive: &mut zip::ZipArchive<std::io::Cursor<Vec<u8>>>,
    sheet_index: usize,
    warnings: &mut Vec<String>,
) -> Vec<ChartModel> {
    let sheet_path = format!("xl/worksheets/sheet{}.xml", sheet_index + 1);
    let Some(sheet_xml) = read_xlsx_entry(archive, &sheet_path) else {
        return Vec::new();
    };
    let Some(drawing_id) = parse_sheet_drawing_id(&sheet_xml) else {
        return Vec::new();
    };
    let Some(sheet_rels_xml) =
        relationship_part_path(&sheet_path).and_then(|path| read_xlsx_entry(archive, &path))
    else {
        return Vec::new();
    };
    let sheet_rels = parse_relationships(&sheet_rels_xml);
    let Some(drawing_target) = sheet_rels.get(&drawing_id) else {
        return Vec::new();
    };
    let Some(drawing_path) = normalize_xlsx_target("xl/worksheets", drawing_target) else {
        return Vec::new();
    };
    let Some(drawing_xml) = read_xlsx_entry(archive, &drawing_path) else {
        warnings.push(format!("sheet {}: drawing was unreadable", sheet_index + 1));
        return Vec::new();
    };
    let Some(drawing_rels_xml) =
        relationship_part_path(&drawing_path).and_then(|path| read_xlsx_entry(archive, &path))
    else {
        return Vec::new();
    };
    let drawing_rels = parse_relationships(&drawing_rels_xml);
    let mut charts = Vec::new();
    for anchor in parse_drawing_anchors(&drawing_xml) {
        if anchor.kind != DrawingAnchorKind::Chart {
            continue;
        }
        let Some(chart_target) = drawing_rels.get(&anchor.relationship) else {
            warnings.push(format!(
                "sheet {}: chart relationship {} was missing",
                sheet_index + 1,
                anchor.relationship
            ));
            continue;
        };
        let Some(chart_path) = normalize_xlsx_target("xl/drawings", chart_target) else {
            continue;
        };
        let Some(chart_xml) = read_xlsx_entry(archive, &chart_path) else {
            warnings.push(format!(
                "sheet {}: chart {chart_path} was unreadable",
                sheet_index + 1
            ));
            continue;
        };
        let (chart_type, title) = parse_chart_metadata(&chart_xml);
        charts.push(ChartModel {
            chart_type,
            title,
            start_row: anchor.start_row.saturating_add(1),
            end_row: anchor.end_row.saturating_add(1),
            start_col: anchor.start_col.saturating_add(1),
            end_col: anchor.end_col.saturating_add(1),
        });
    }
    charts
}

fn xlsx_image_mime(path: &str) -> Option<&'static str> {
    match path
        .rsplit_once('.')
        .map(|(_, ext)| ext.to_ascii_lowercase())
        .as_deref()
    {
        Some("png") => Some("image/png"),
        Some("jpg") | Some("jpeg") => Some("image/jpeg"),
        Some("gif") => Some("image/gif"),
        Some("webp") => Some("image/webp"),
        _ => None,
    }
}

fn import_sheet_images(
    archive: &mut zip::ZipArchive<std::io::Cursor<Vec<u8>>>,
    sheet_index: usize,
    warnings: &mut Vec<String>,
    total_media_bytes: &mut u64,
) -> Vec<(u32, u32, String)> {
    let sheet_path = format!("xl/worksheets/sheet{}.xml", sheet_index + 1);
    let Some(sheet_xml) = read_xlsx_entry(archive, &sheet_path) else {
        return Vec::new();
    };
    let Some(drawing_id) = parse_sheet_drawing_id(&sheet_xml) else {
        return Vec::new();
    };
    let Some(sheet_rels_xml) =
        relationship_part_path(&sheet_path).and_then(|path| read_xlsx_entry(archive, &path))
    else {
        return Vec::new();
    };
    let sheet_rels = parse_relationships(&sheet_rels_xml);
    let Some(drawing_target) = sheet_rels.get(&drawing_id) else {
        return Vec::new();
    };
    let Some(drawing_path) = normalize_xlsx_target("xl/worksheets", drawing_target) else {
        return Vec::new();
    };
    let Some(drawing_xml) = read_xlsx_entry(archive, &drawing_path) else {
        warnings.push(format!("sheet {}: drawing was unreadable", sheet_index + 1));
        return Vec::new();
    };
    let Some(drawing_rels_xml) =
        relationship_part_path(&drawing_path).and_then(|path| read_xlsx_entry(archive, &path))
    else {
        return Vec::new();
    };
    let drawing_rels = parse_relationships(&drawing_rels_xml);
    let mut images = Vec::new();
    for anchor in parse_drawing_anchors(&drawing_xml) {
        if anchor.kind != DrawingAnchorKind::Image {
            continue;
        }
        let Some(media_target) = drawing_rels.get(&anchor.relationship) else {
            warnings.push(format!(
                "sheet {}: image relationship {} was missing",
                sheet_index + 1,
                anchor.relationship
            ));
            continue;
        };
        let Some(media_path) = normalize_xlsx_target("xl/drawings", media_target) else {
            continue;
        };
        let Some(mime) = xlsx_image_mime(&media_path) else {
            warnings.push(format!(
                "sheet {}: unsupported image format {media_path}",
                sheet_index + 1
            ));
            continue;
        };
        let Some(bytes) = read_xlsx_entry(archive, &media_path) else {
            warnings.push(format!(
                "sheet {}: image {media_path} was unreadable",
                sheet_index + 1
            ));
            continue;
        };
        if bytes.len() > MAX_INLINE_IMAGE_BYTES {
            warnings.push(format!(
                "sheet {}: image {media_path} exceeds the {} byte limit",
                sheet_index + 1,
                MAX_INLINE_IMAGE_BYTES
            ));
            continue;
        }
        if total_media_bytes.saturating_add(bytes.len() as u64) > MAX_TOTAL_INLINE_IMAGE_BYTES {
            warnings.push(format!(
                "sheet {}: image {media_path} skipped because the total image limit was exceeded",
                sheet_index + 1
            ));
            continue;
        }
        *total_media_bytes = total_media_bytes.saturating_add(bytes.len() as u64);
        images.push((
            anchor.start_row.saturating_add(1),
            anchor.start_col.saturating_add(1),
            format!("data:{mime};base64,{}", encode_base64(&bytes)),
        ));
    }
    images
}

pub fn import_workbook_from_xlsx(path: &Path) -> Result<WorkbookModel, ExportError> {
    Ok(import_workbook_from_xlsx_with_report(path)?.workbook)
}

pub fn import_workbook_from_xlsx_with_report(path: &Path) -> Result<XlsxImportResult, ExportError> {
    let file_size = std::fs::metadata(path)?.len();
    if file_size > MAX_XLSX_FILE_BYTES {
        return Err(ExportError::Xlsx(format!(
            "XLSX file exceeds the {MAX_XLSX_FILE_BYTES} byte limit"
        )));
    }
    let mut source =
        open_workbook_auto(path).map_err(|error| ExportError::Xlsx(error.to_string()))?;
    let mut archive = std::fs::read(path)
        .ok()
        .and_then(|bytes| zip::ZipArchive::new(std::io::Cursor::new(bytes)).ok());
    if archive
        .as_ref()
        .is_some_and(|archive| archive.len() > MAX_XLSX_ARCHIVE_ENTRIES)
    {
        return Err(ExportError::Xlsx(format!(
            "XLSX archive has too many entries (maximum {MAX_XLSX_ARCHIVE_ENTRIES})"
        )));
    }
    let mut warnings = Vec::new();
    let mut total_media_bytes = 0u64;
    let has_chart_parts = archive.as_mut().map(archive_has_charts).unwrap_or(false);
    let sheet_names = source.sheet_names().to_owned();
    let named_ranges = archive
        .as_mut()
        .map(|archive| parse_xlsx_named_ranges(archive, &sheet_names))
        .unwrap_or_default();
    let mut sheets = Vec::new();
    for (sheet_index, name) in sheet_names.into_iter().enumerate() {
        let range = source
            .worksheet_range(&name)
            .map_err(|error| ExportError::Xlsx(error.to_string()))?;
        let formulas = source
            .worksheet_formula(&name)
            .map_err(|error| ExportError::Xlsx(error.to_string()))?;
        let mut sheet = SheetData::new(&format!("sheet-{}", sheet_index + 1), &name);
        let (origin_row, origin_col) = range.start().unwrap_or((0, 0));
        for (row, values) in range.rows().enumerate() {
            for (column, value) in values.iter().enumerate() {
                let actual_row = origin_row.saturating_add(row as u32);
                let actual_col = origin_col.saturating_add(column as u32);
                let text = match value {
                    Data::Empty => String::new(),
                    Data::String(value) => value.clone(),
                    Data::Float(value) => value.to_string(),
                    Data::Int(value) => value.to_string(),
                    Data::Bool(value) => value.to_string().to_uppercase(),
                    other => other.to_string(),
                };
                if text.is_empty() {
                    continue;
                }
                let formula = formulas
                    .get_value((actual_row, actual_col))
                    .filter(|formula| !formula.is_empty())
                    .map(|formula| {
                        if formula.starts_with('=') {
                            formula.clone()
                        } else {
                            format!("={formula}")
                        }
                    });
                let raw_value = formula.clone().unwrap_or_else(|| text.clone());
                sheet.cells.insert(
                    format!("{}:{}", actual_row + 1, actual_col + 1),
                    SheetCell {
                        raw_value,
                        display_value: text,
                        formula,
                        style: None,
                    },
                );
            }
        }
        if let Some(archive) = archive.as_mut() {
            let metadata = parse_xlsx_sheet_metadata(archive, sheet_index);
            sheet.col_widths = metadata.col_widths.clone();
            sheet.row_heights = metadata.row_heights.clone();
            sheet.freeze_rows = metadata.freeze_rows;
            sheet.freeze_cols = metadata.freeze_cols;
            sheet.auto_filter = metadata.auto_filter.clone();
            sheet.merges = metadata.merges.clone();
            sheet.conditional_formatting = metadata.conditional_formatting.clone();
            sheet.tables = metadata.tables.clone();
            sheet.scenarios = metadata.scenarios.clone();
            let metadata_only_cells = metadata
                .validations
                .keys()
                .chain(metadata.hyperlinks.keys())
                .filter(|key| !sheet.cells.contains_key(*key))
                .cloned()
                .collect::<Vec<_>>();
            for key in metadata_only_cells {
                sheet.cells.insert(
                    key,
                    SheetCell {
                        raw_value: String::new(),
                        display_value: String::new(),
                        formula: None,
                        style: None,
                    },
                );
            }
            for (key, cell) in &mut sheet.cells {
                if let Ok((row, column)) = redoc_sheet_engine::parse_key(key) {
                    cell.style = imported_style(&metadata, &excel_reference(row, column));
                }
                if let Some(validation) = metadata.validations.get(key) {
                    if let Some(style) = cell.style.as_mut() {
                        style.validation = Some(validation.clone());
                    } else {
                        cell.style = Some(CellStyle {
                            bold: None,
                            italic: None,
                            underline: None,
                            font_color: None,
                            bg_color: None,
                            align: None,
                            format: None,
                            wrap: None,
                            v_align: None,
                            validation: Some(validation.clone()),
                            hyperlink: None,
                            image: None,
                            ..Default::default()
                        });
                    }
                }
                if let Some(hyperlink) = metadata.hyperlinks.get(key) {
                    if let Some(style) = cell.style.as_mut() {
                        style.hyperlink = Some(hyperlink.clone());
                    } else {
                        cell.style = Some(CellStyle {
                            bold: None,
                            italic: None,
                            underline: None,
                            font_color: None,
                            bg_color: None,
                            align: None,
                            format: None,
                            wrap: None,
                            v_align: None,
                            validation: metadata.validations.get(key).cloned(),
                            hyperlink: Some(hyperlink.clone()),
                            image: None,
                            ..Default::default()
                        });
                    }
                }
            }
            for (row, column, image) in
                import_sheet_images(archive, sheet_index, &mut warnings, &mut total_media_bytes)
            {
                let key = format!("{row}:{column}");
                let cell = sheet.cells.entry(key).or_insert_with(|| SheetCell {
                    raw_value: String::new(),
                    display_value: String::new(),
                    formula: None,
                    style: None,
                });
                let style = cell.style.get_or_insert(CellStyle {
                    bold: None,
                    italic: None,
                    underline: None,
                    font_color: None,
                    bg_color: None,
                    align: None,
                    format: None,
                    wrap: None,
                    v_align: None,
                    validation: None,
                    hyperlink: None,
                    image: None,
                    ..Default::default()
                });
                style.image = Some(image);
            }
            sheet.charts = import_sheet_charts(archive, sheet_index, &mut warnings);
        }
        sheets.push(sheet);
    }
    if let Some(archive) = archive.as_mut() {
        let names = sheets
            .iter()
            .map(|sheet| sheet.name.clone())
            .collect::<Vec<_>>();
        let pivots = crate::xlsx_pivot::import_pivots_from_xlsx(archive, &names, &mut warnings);
        for (sheet, sheet_pivots) in sheets.iter_mut().zip(pivots) {
            sheet.pivot_tables = sheet_pivots;
        }
    }
    if has_chart_parts && sheets.iter().all(|sheet| sheet.charts.is_empty()) {
        warnings.push(
            "Charts were present but could not be mapped to worksheet anchors; chart objects were skipped."
                .to_string(),
        );
    }
    Ok(XlsxImportResult {
        workbook: WorkbookModel {
            sheets,
            active_sheet_index: 0,
            formula_cache: std::collections::HashMap::new(),
            cell_value_cache: Vec::new(),
            cell_value_cache_valid: Vec::new(),
            formula_values_valid: Vec::new(),
            recalc_plans: Vec::new(),
            named_ranges,
            cross_sheet_dependents: std::collections::HashMap::new(),
            cross_sheet_index_valid: false,
        },
        warnings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use redoc_sheet_engine::{ConditionalFormattingRange, ScenarioCellChange};
    use std::io::Read;

    #[test]
    fn xlsx_round_trip_reads_values() {
        let path = std::env::temp_dir().join(format!(
            "redoc-xlsx-{}-{}.xlsx",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut workbook = WorkbookModel::new_default();
        workbook.sheets[0].cells.insert(
            "1:1".to_string(),
            SheetCell {
                raw_value: "42".to_string(),
                display_value: "42".to_string(),
                formula: None,
                style: None,
            },
        );
        std::fs::write(
            &path,
            export_workbook_to_xlsx(&workbook).expect("export xlsx"),
        )
        .expect("write xlsx");
        let loaded = import_workbook_from_xlsx(&path).expect("import xlsx");
        assert_eq!(loaded.sheets[0].cells["1:1"].raw_value, "42");
        std::fs::remove_file(path).expect("cleanup xlsx");
    }

    #[test]
    fn xlsx_export_emits_native_png_cell_images() {
        let path = std::env::temp_dir().join(format!(
            "redoc-xlsx-image-{}-{}.xlsx",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut workbook = WorkbookModel::new_default();
        workbook.sheets[0].cells.insert(
            "1:1".to_string(),
            SheetCell {
                raw_value: "logo".to_string(),
                display_value: "logo".to_string(),
                formula: None,
                style: Some(CellStyle {
                    bold: None,
                    italic: None,
                    underline: None,
                    font_color: None,
                    bg_color: None,
                    align: None,
                    format: None,
                    wrap: None,
                    v_align: None,
                    validation: None,
                    hyperlink: None,
                    image: Some("data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=".to_string()),
                    ..Default::default()
                }),
            },
        );
        let bytes = export_workbook_to_xlsx(&workbook).expect("export xlsx image");
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).expect("read xlsx zip");
        assert!(archive.by_name("xl/media/image1.png").is_ok());
        assert!(archive.by_name("xl/drawings/drawing1.xml").is_ok());
        let mut sheet_xml = String::new();
        archive
            .by_name("xl/worksheets/sheet1.xml")
            .expect("worksheet part")
            .read_to_string(&mut sheet_xml)
            .expect("read worksheet");
        assert!(sheet_xml.contains("<drawing"));
        std::fs::write(
            &path,
            export_workbook_to_xlsx(&workbook).expect("write xlsx image"),
        )
        .expect("write image workbook");
        let imported = import_workbook_from_xlsx_with_report(&path).expect("import xlsx image");
        assert!(imported.workbook.sheets[0].cells["1:1"]
            .style
            .as_ref()
            .and_then(|style| style.image.as_deref())
            .is_some_and(|image| image.starts_with("data:image/png;base64,")));
        std::fs::remove_file(path).expect("cleanup xlsx image");
    }

    #[test]
    fn xlsx_round_trip_preserves_scenarios() {
        let path = std::env::temp_dir().join(format!(
            "redoc-xlsx-scenarios-{}-{}.xlsx",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut workbook = WorkbookModel::new_default();
        workbook.sheets[0].scenarios.push(ScenarioModel {
            id: "scenario-1".to_string(),
            name: "Best & quoted".to_string(),
            changes: vec![
                ScenarioCellChange {
                    row: 2,
                    col: 4,
                    raw_value: "151".to_string(),
                },
                ScenarioCellChange {
                    row: 2,
                    col: 6,
                    raw_value: "A & \"quoted\"".to_string(),
                },
            ],
        });
        let bytes = export_workbook_to_xlsx(&workbook).expect("export xlsx scenarios");
        let mut archive =
            zip::ZipArchive::new(std::io::Cursor::new(bytes.clone())).expect("read xlsx zip");
        let mut sheet_xml = String::new();
        archive
            .by_name("xl/worksheets/sheet1.xml")
            .expect("worksheet part")
            .read_to_string(&mut sheet_xml)
            .expect("read worksheet");
        assert!(sheet_xml.contains("<scenarios"));
        assert!(sheet_xml.contains("name=\"Best &amp; quoted\""));
        assert!(sheet_xml.contains("r=\"D2\" val=\"151\""));
        assert!(sheet_xml.contains("r=\"F2\" val=\"A &amp; &quot;quoted&quot;\""));
        std::fs::write(&path, bytes).expect("write xlsx");

        let imported = import_workbook_from_xlsx_with_report(&path).expect("import xlsx scenarios");
        assert_eq!(imported.workbook.sheets[0].scenarios.len(), 1);
        assert_eq!(
            imported.workbook.sheets[0].scenarios[0].name,
            "Best & quoted"
        );
        assert_eq!(
            imported.workbook.sheets[0].scenarios[0].changes,
            workbook.sheets[0].scenarios[0].changes
        );
        std::fs::remove_file(path).expect("cleanup xlsx");
    }

    #[test]
    fn xlsx_round_trip_preserves_list_validation() {
        let path = std::env::temp_dir().join(format!(
            "redoc-xlsx-validation-{}-{}.xlsx",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut workbook = WorkbookModel::new_default();
        workbook.sheets[0].cells.insert(
            "2:1".to_string(),
            SheetCell {
                raw_value: "Open".to_string(),
                display_value: "Open".to_string(),
                formula: None,
                style: Some(CellStyle {
                    bold: None,
                    italic: None,
                    underline: None,
                    font_color: None,
                    bg_color: None,
                    align: None,
                    format: None,
                    wrap: None,
                    v_align: None,
                    validation: Some(ListValidation {
                        validation_type: "list".to_string(),
                        options: vec!["Open".to_string(), "Closed".to_string()],
                        formula: None,
                    }),
                    hyperlink: None,
                    image: None,
                    ..Default::default()
                }),
            },
        );
        std::fs::write(
            &path,
            export_workbook_to_xlsx(&workbook).expect("export xlsx"),
        )
        .expect("write xlsx");
        let mut archive = zip::ZipArchive::new(std::fs::File::open(&path).expect("open xlsx"))
            .expect("read xlsx zip");
        let mut xml = String::new();
        archive
            .by_name("xl/worksheets/sheet1.xml")
            .expect("worksheet part")
            .read_to_string(&mut xml)
            .expect("read worksheet");
        assert!(xml.contains("dataValidation"));
        assert!(xml.contains("formula1>\"Open,Closed\""));

        let imported = import_workbook_from_xlsx_with_report(&path).expect("import xlsx");
        let validation = imported.workbook.sheets[0].cells["2:1"]
            .style
            .as_ref()
            .and_then(|style| style.validation.as_ref())
            .expect("list validation should be imported");
        assert_eq!(validation.validation_type, "list");
        assert_eq!(validation.options, ["Open", "Closed"]);
        std::fs::remove_file(path).expect("cleanup xlsx");
    }

    #[test]
    fn xlsx_round_trip_preserves_formula_list_validation() {
        let path = std::env::temp_dir().join(format!(
            "redoc-xlsx-formula-validation-{}-{}.xlsx",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut workbook = WorkbookModel::new_default();
        for (key, value) in [("1:2", "Open"), ("2:2", "Closed")] {
            workbook.sheets[0].cells.insert(
                key.to_string(),
                SheetCell {
                    raw_value: value.to_string(),
                    display_value: value.to_string(),
                    formula: None,
                    style: None,
                },
            );
        }
        workbook.sheets[0].cells.insert(
            "1:3".to_string(),
            SheetCell {
                raw_value: "Open".to_string(),
                display_value: "Open".to_string(),
                formula: None,
                style: Some(CellStyle {
                    bold: None,
                    italic: None,
                    underline: None,
                    font_color: None,
                    bg_color: None,
                    align: None,
                    format: None,
                    wrap: None,
                    v_align: None,
                    validation: Some(ListValidation {
                        validation_type: "list".to_string(),
                        options: Vec::new(),
                        formula: Some("=$B$1:$B$2".to_string()),
                    }),
                    hyperlink: None,
                    image: None,
                    ..Default::default()
                }),
            },
        );
        std::fs::write(
            &path,
            export_workbook_to_xlsx(&workbook).expect("export xlsx"),
        )
        .expect("write xlsx");
        let mut archive = zip::ZipArchive::new(std::fs::File::open(&path).expect("open xlsx"))
            .expect("read xlsx zip");
        let mut xml = String::new();
        archive
            .by_name("xl/worksheets/sheet1.xml")
            .expect("worksheet part")
            .read_to_string(&mut xml)
            .expect("read worksheet");
        assert!(xml.contains("formula1>$B$1:$B$2</formula1>"));

        let imported = import_workbook_from_xlsx_with_report(&path).expect("import xlsx");
        let validation = imported.workbook.sheets[0].cells["1:3"]
            .style
            .as_ref()
            .and_then(|style| style.validation.as_ref())
            .expect("formula validation should be imported");
        assert!(validation.options.is_empty());
        assert_eq!(validation.formula.as_deref(), Some("$B$1:$B$2"));
        std::fs::remove_file(path).expect("cleanup xlsx");
    }

    #[test]
    fn xlsx_round_trip_preserves_hyperlinks() {
        let path = std::env::temp_dir().join(format!(
            "redoc-xlsx-hyperlink-{}-{}.xlsx",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut workbook = WorkbookModel::new_default();
        workbook.sheets[0].cells.insert(
            "1:1".to_string(),
            SheetCell {
                raw_value: "Open site".to_string(),
                display_value: "Open site".to_string(),
                formula: None,
                style: Some(CellStyle {
                    bold: None,
                    italic: None,
                    underline: None,
                    font_color: None,
                    bg_color: None,
                    align: None,
                    format: None,
                    wrap: None,
                    v_align: None,
                    validation: None,
                    hyperlink: Some("https://example.com/docs".to_string()),
                    image: None,
                    ..Default::default()
                }),
            },
        );
        workbook.sheets[0].cells.insert(
            "1:2".to_string(),
            SheetCell {
                raw_value: "Go to A1".to_string(),
                display_value: "Go to A1".to_string(),
                formula: None,
                style: Some(CellStyle {
                    bold: None,
                    italic: None,
                    underline: None,
                    font_color: None,
                    bg_color: None,
                    align: None,
                    format: None,
                    wrap: None,
                    v_align: None,
                    validation: None,
                    hyperlink: Some("internal:Sheet1!A1".to_string()),
                    image: None,
                    ..Default::default()
                }),
            },
        );
        std::fs::write(
            &path,
            export_workbook_to_xlsx(&workbook).expect("export xlsx"),
        )
        .expect("write xlsx");
        let mut archive = zip::ZipArchive::new(std::fs::File::open(&path).expect("open xlsx"))
            .expect("read xlsx zip");
        let mut sheet_xml = String::new();
        archive
            .by_name("xl/worksheets/sheet1.xml")
            .expect("worksheet part")
            .read_to_string(&mut sheet_xml)
            .expect("read worksheet");
        assert!(sheet_xml.contains("<hyperlinks>"));
        assert!(sheet_xml.contains("ref=\"A1\""));
        assert!(sheet_xml.contains("ref=\"B1\""));
        assert!(sheet_xml.contains("location=\"Sheet1!A1\""));
        assert!(sheet_xml.contains("r:id=\"rId1\""));
        let mut rels_xml = String::new();
        archive
            .by_name("xl/worksheets/_rels/sheet1.xml.rels")
            .expect("worksheet relationships")
            .read_to_string(&mut rels_xml)
            .expect("read relationships");
        assert!(rels_xml.contains("Target=\"https://example.com/docs\""));

        let imported = import_workbook_from_xlsx_with_report(&path).expect("import xlsx");
        assert_eq!(
            imported.workbook.sheets[0].cells["1:1"]
                .style
                .as_ref()
                .and_then(|style| style.hyperlink.as_deref()),
            Some("https://example.com/docs")
        );
        assert_eq!(
            imported.workbook.sheets[0].cells["1:2"]
                .style
                .as_ref()
                .and_then(|style| style.hyperlink.as_deref()),
            Some("internal:Sheet1!A1")
        );
        std::fs::remove_file(path).expect("cleanup xlsx");
    }

    #[test]
    fn xlsx_export_rejects_malformed_hyperlinks() {
        let mut workbook = WorkbookModel::new_default();
        workbook.sheets[0].cells.insert(
            "1:1".to_string(),
            SheetCell {
                raw_value: "Unsafe".to_string(),
                display_value: "Unsafe".to_string(),
                formula: None,
                style: Some(CellStyle {
                    bold: None,
                    italic: None,
                    underline: None,
                    font_color: None,
                    bg_color: None,
                    align: None,
                    format: None,
                    wrap: None,
                    v_align: None,
                    validation: None,
                    hyperlink: Some("javascript:alert(1)".to_string()),
                    image: None,
                    ..Default::default()
                }),
            },
        );
        let error = export_workbook_to_xlsx(&workbook).expect_err("invalid URL must fail");
        assert!(error.to_string().to_ascii_lowercase().contains("hyperlink"));

        workbook.sheets[0]
            .cells
            .get_mut("1:1")
            .unwrap()
            .style
            .as_mut()
            .unwrap()
            .hyperlink = Some("https://".to_string());
        let error = export_workbook_to_xlsx(&workbook).expect_err("empty web host must fail");
        assert!(error.to_string().contains("hyperlink target"));

        workbook.sheets[0]
            .cells
            .get_mut("1:1")
            .unwrap()
            .style
            .as_mut()
            .unwrap()
            .hyperlink = Some("internal:javascript:alert(1)".to_string());
        let error =
            export_workbook_to_xlsx(&workbook).expect_err("invalid internal target must fail");
        assert!(error.to_string().contains("hyperlink target"));

        workbook.sheets[0]
            .cells
            .get_mut("1:1")
            .unwrap()
            .style
            .as_mut()
            .unwrap()
            .hyperlink = Some("internal:".to_string());
        let error =
            export_workbook_to_xlsx(&workbook).expect_err("empty internal target must fail");
        assert!(error.to_string().contains("hyperlink target"));
    }

    #[test]
    fn xlsx_round_trip_preserves_table_metadata() {
        let path = std::env::temp_dir().join(format!(
            "redoc-xlsx-table-{}-{}.xlsx",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut workbook = WorkbookModel::new_default();
        for (key, value) in [
            ("1:1", "Region"),
            ("1:2", "Sales"),
            ("2:1", "East"),
            ("2:2", "10"),
            ("3:1", "West"),
            ("3:2", "20"),
        ] {
            workbook.sheets[0].cells.insert(
                key.to_string(),
                SheetCell {
                    raw_value: value.to_string(),
                    display_value: value.to_string(),
                    formula: None,
                    style: None,
                },
            );
        }
        workbook.sheets[0]
            .tables
            .push(redoc_sheet_engine::TableModel {
                id: "1".to_string(),
                name: "SalesTable".to_string(),
                range: redoc_sheet_engine::CellRange {
                    start_row: 1,
                    end_row: 3,
                    start_col: 1,
                    end_col: 2,
                },
                columns: vec!["Region".to_string(), "Sales".to_string()],
                style: Some("TableStyleMedium2".to_string()),
                show_header_row: true,
                show_total_row: false,
            });
        std::fs::write(
            &path,
            export_workbook_to_xlsx(&workbook).expect("export xlsx"),
        )
        .expect("write xlsx");
        let mut archive = zip::ZipArchive::new(std::fs::File::open(&path).expect("open xlsx"))
            .expect("read xlsx zip");
        let mut sheet_xml = String::new();
        archive
            .by_name("xl/worksheets/sheet1.xml")
            .expect("worksheet part")
            .read_to_string(&mut sheet_xml)
            .expect("read worksheet");
        assert!(sheet_xml.contains("<tableParts"));
        assert!(sheet_xml.contains("tablePart"));
        let mut table_xml = String::new();
        archive
            .by_name("xl/tables/table1.xml")
            .expect("table part")
            .read_to_string(&mut table_xml)
            .expect("read table");
        assert!(table_xml.contains("displayName=\"SalesTable\""));
        assert!(table_xml.contains("ref=\"A1:B3\""));
        assert!(table_xml.contains("TableStyleMedium2"));

        let imported = import_workbook_from_xlsx_with_report(&path).expect("import xlsx");
        assert_eq!(imported.workbook.sheets[0].tables.len(), 1);
        let table = &imported.workbook.sheets[0].tables[0];
        assert_eq!(table.name, "SalesTable");
        assert_eq!(table.range.start_row, 1);
        assert_eq!(table.range.end_col, 2);
        assert_eq!(table.columns, ["Region", "Sales"]);
        assert_eq!(table.style.as_deref(), Some("TableStyleMedium2"));
        std::fs::remove_file(path).expect("cleanup xlsx");
    }

    #[test]
    fn xlsx_export_preserves_sheet_layout_and_cell_style() {
        let mut workbook = WorkbookModel::new_default();
        let sheet = &mut workbook.sheets[0];
        sheet.freeze_rows = 1;
        sheet.freeze_cols = 1;
        sheet.col_widths.insert(1, 24.0);
        sheet.row_heights.insert(1, 28.0);
        sheet.cells.insert(
            "1:1".to_string(),
            SheetCell {
                raw_value: "42".to_string(),
                display_value: "42".to_string(),
                formula: None,
                style: Some(CellStyle {
                    bold: Some(true),
                    italic: None,
                    underline: None,
                    font_color: Some("#FF0000".to_string()),
                    bg_color: Some("#FFFF00".to_string()),
                    align: Some("center".to_string()),
                    format: Some("number".to_string()),
                    wrap: None,
                    v_align: None,
                    validation: None,
                    hyperlink: None,
                    image: None,
                    ..Default::default()
                }),
            },
        );
        let bytes = export_workbook_to_xlsx(&workbook).expect("export xlsx");
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).expect("read xlsx zip");
        let mut sheet_xml = String::new();
        archive
            .by_name("xl/worksheets/sheet1.xml")
            .expect("sheet xml")
            .read_to_string(&mut sheet_xml)
            .expect("read sheet xml");
        assert!(sheet_xml.contains("xSplit=\"1\""));
        assert!(sheet_xml.contains("ySplit=\"1\""));
        assert!(sheet_xml.contains("customWidth=\"1\""));
        assert!(sheet_xml.contains("ht=\"28\""));
        assert!(sheet_xml.contains("s=\"1\""));
    }

    #[test]
    fn xlsx_round_trip_preserves_merged_ranges() {
        let path = std::env::temp_dir().join(format!(
            "redoc-xlsx-merge-{}-{}.xlsx",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut workbook = WorkbookModel::new_default();
        workbook.sheets[0].merges.push(MergeRange {
            start_row: 1,
            end_row: 2,
            start_col: 1,
            end_col: 3,
        });
        std::fs::write(
            &path,
            export_workbook_to_xlsx(&workbook).expect("export xlsx"),
        )
        .expect("write xlsx");
        let imported = import_workbook_from_xlsx_with_report(&path).expect("import xlsx");
        assert_eq!(
            imported.workbook.sheets[0].merges,
            vec![MergeRange {
                start_row: 1,
                end_row: 2,
                start_col: 1,
                end_col: 3,
            }]
        );
        std::fs::remove_file(path).expect("cleanup xlsx");
    }

    #[test]
    fn xlsx_import_preserves_formulas() {
        let mut workbook = WorkbookModel::new_default();
        workbook.sheets[0].cells.insert(
            "1:1".to_string(),
            SheetCell {
                raw_value: "2".to_string(),
                display_value: "2".to_string(),
                formula: None,
                style: None,
            },
        );
        workbook.sheets[0].cells.insert(
            "1:2".to_string(),
            SheetCell {
                raw_value: "=A1*2".to_string(),
                display_value: "4".to_string(),
                formula: Some("=A1*2".to_string()),
                style: None,
            },
        );
        let path = std::env::temp_dir().join(format!(
            "redoc-xlsx-formula-{}-{}.xlsx",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::write(
            &path,
            export_workbook_to_xlsx(&workbook).expect("export xlsx"),
        )
        .expect("write xlsx");
        let loaded = import_workbook_from_xlsx(&path).expect("import xlsx");
        assert_eq!(
            loaded.sheets[0].cells["1:2"].formula.as_deref(),
            Some("=A1*2")
        );
        std::fs::remove_file(path).expect("cleanup xlsx");
    }

    #[test]
    fn xlsx_round_trip_preserves_autofilter_range_and_selected_values() {
        let path = std::env::temp_dir().join(format!(
            "redoc-xlsx-filter-{}-{}.xlsx",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut workbook = WorkbookModel::new_default();
        let sheet = &mut workbook.sheets[0];
        for (key, value) in [
            ("1:1", "Region"),
            ("1:2", "Sales"),
            ("2:1", "East"),
            ("2:2", "10"),
            ("3:1", "West"),
            ("3:2", "20"),
        ] {
            sheet.cells.insert(
                key.to_string(),
                SheetCell {
                    raw_value: value.to_string(),
                    display_value: value.to_string(),
                    formula: None,
                    style: None,
                },
            );
        }
        sheet.auto_filter = Some(AutoFilterState {
            enabled: true,
            start_row: 1,
            end_row: 3,
            start_col: 1,
            end_col: 2,
            column_filters: BTreeMap::from([(1, vec!["East".to_string()])]),
        });
        std::fs::write(
            &path,
            export_workbook_to_xlsx(&workbook).expect("export xlsx"),
        )
        .expect("write xlsx");

        let mut archive = zip::ZipArchive::new(std::fs::File::open(&path).expect("open xlsx"))
            .expect("read xlsx zip");
        let mut xml = String::new();
        archive
            .by_name("xl/worksheets/sheet1.xml")
            .expect("worksheet part")
            .read_to_string(&mut xml)
            .expect("read worksheet");
        assert!(xml.contains("autoFilter ref=\"A1:B3\""));
        assert!(xml.contains("filterColumn colId=\"0\""));
        assert!(xml.contains("<filter val=\"East\""));

        let imported = import_workbook_from_xlsx_with_report(&path).expect("import xlsx");
        let filter = imported.workbook.sheets[0]
            .auto_filter
            .as_ref()
            .expect("autofilter should be imported");
        assert_eq!(
            (
                filter.start_row,
                filter.end_row,
                filter.start_col,
                filter.end_col
            ),
            (1, 3, 1, 2)
        );
        assert_eq!(
            filter.column_filters.get(&1),
            Some(&vec!["East".to_string()])
        );
        std::fs::remove_file(path).expect("cleanup xlsx");
    }

    #[test]
    fn xlsx_round_trip_preserves_named_ranges() {
        let path = std::env::temp_dir().join(format!(
            "redoc-xlsx-named-range-{}-{}.xlsx",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut workbook = WorkbookModel::new_default();
        workbook.named_ranges.push(NamedRange {
            name: "Revenue".to_string(),
            range_str: "A1:B3".to_string(),
            sheet: Some(workbook.sheets[0].name.clone()),
        });
        std::fs::write(
            &path,
            export_workbook_to_xlsx(&workbook).expect("export xlsx"),
        )
        .expect("write xlsx");

        let mut archive = zip::ZipArchive::new(std::fs::File::open(&path).expect("open xlsx"))
            .expect("read xlsx zip");
        let mut workbook_xml = String::new();
        archive
            .by_name("xl/workbook.xml")
            .expect("workbook part")
            .read_to_string(&mut workbook_xml)
            .expect("read workbook");
        assert!(workbook_xml.contains("name=\"Revenue\""));
        assert!(workbook_xml.contains("A1:B3"));

        let imported = import_workbook_from_xlsx_with_report(&path).expect("import xlsx");
        assert_eq!(imported.workbook.named_ranges.len(), 1);
        assert_eq!(imported.workbook.named_ranges[0].name, "Revenue");
        assert_eq!(imported.workbook.named_ranges[0].range_str, "A1:B3");
        assert_eq!(
            imported.workbook.named_ranges[0].sheet.as_deref(),
            Some("Sheet1")
        );
        std::fs::remove_file(path).expect("cleanup xlsx");
    }

    #[test]
    fn xlsx_import_preserves_layout_and_number_format() {
        let mut workbook = WorkbookModel::new_default();
        workbook.sheets[0].freeze_rows = 2;
        workbook.sheets[0].freeze_cols = 1;
        workbook.sheets[0].col_widths.insert(1, 22.0);
        workbook.sheets[0].row_heights.insert(1, 27.0);
        workbook.sheets[0].cells.insert(
            "1:1".to_string(),
            SheetCell {
                raw_value: "0.5".to_string(),
                display_value: "0.5".to_string(),
                formula: None,
                style: Some(CellStyle {
                    bold: Some(true),
                    italic: None,
                    underline: None,
                    font_color: Some("#FF0000".to_string()),
                    bg_color: Some("#FFFF00".to_string()),
                    align: Some("right".to_string()),
                    format: Some("percent".to_string()),
                    wrap: None,
                    v_align: None,
                    validation: None,
                    hyperlink: None,
                    image: None,
                    ..Default::default()
                }),
            },
        );
        let path = std::env::temp_dir().join(format!(
            "redoc-xlsx-metadata-{}-{}.xlsx",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::write(
            &path,
            export_workbook_to_xlsx(&workbook).expect("export xlsx"),
        )
        .expect("write xlsx");
        let loaded = import_workbook_from_xlsx(&path).expect("import xlsx");
        let sheet = &loaded.sheets[0];
        assert_eq!(sheet.freeze_rows, 2);
        assert_eq!(sheet.freeze_cols, 1);
        assert!(sheet.col_widths.contains_key(&1));
        assert!(sheet.row_heights.contains_key(&1));
        assert_eq!(
            sheet.cells["1:1"]
                .style
                .as_ref()
                .and_then(|style| style.format.as_deref()),
            Some("percent")
        );
        assert_eq!(
            sheet.cells["1:1"]
                .style
                .as_ref()
                .and_then(|style| style.align.as_deref()),
            Some("right")
        );
        assert_eq!(
            sheet.cells["1:1"]
                .style
                .as_ref()
                .and_then(|style| style.bold),
            Some(true)
        );
        assert_eq!(
            sheet.cells["1:1"]
                .style
                .as_ref()
                .and_then(|style| style.font_color.as_deref()),
            Some("#FF0000")
        );
        assert_eq!(
            sheet.cells["1:1"]
                .style
                .as_ref()
                .and_then(|style| style.bg_color.as_deref()),
            Some("#FFFF00")
        );
        std::fs::remove_file(path).expect("cleanup xlsx");
    }

    #[test]
    fn xlsx_export_writes_bar_chart_parts() {
        let mut workbook = WorkbookModel::new_default();
        workbook.sheets[0].cells.insert(
            "1:1".to_string(),
            SheetCell {
                raw_value: "A".to_string(),
                display_value: "A".to_string(),
                formula: None,
                style: None,
            },
        );
        workbook.sheets[0].cells.insert(
            "1:2".to_string(),
            SheetCell {
                raw_value: "10".to_string(),
                display_value: "10".to_string(),
                formula: None,
                style: None,
            },
        );
        workbook.sheets[0].cells.insert(
            "2:1".to_string(),
            SheetCell {
                raw_value: "B".to_string(),
                display_value: "B".to_string(),
                formula: None,
                style: None,
            },
        );
        workbook.sheets[0].cells.insert(
            "2:2".to_string(),
            SheetCell {
                raw_value: "20".to_string(),
                display_value: "20".to_string(),
                formula: None,
                style: None,
            },
        );
        workbook.sheets[0].charts.push(ChartModel {
            chart_type: "bar".to_string(),
            title: Some("Sales".to_string()),
            start_row: 1,
            end_row: 2,
            start_col: 1,
            end_col: 2,
        });
        let bytes = export_workbook_to_xlsx(&workbook).expect("export xlsx with chart");
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).expect("read xlsx zip");
        assert!(
            archive.by_name("xl/charts/chart1.xml").is_ok(),
            "expected OOXML chart part"
        );
        let mut chart_xml = String::new();
        archive
            .by_name("xl/charts/chart1.xml")
            .expect("chart xml")
            .read_to_string(&mut chart_xml)
            .expect("read chart");
        assert!(
            chart_xml.contains("barChart") || chart_xml.contains("c:barChart"),
            "expected bar chart markup"
        );
    }

    #[test]
    fn xlsx_export_writes_conditional_format_rules() {
        let mut workbook = WorkbookModel::new_default();
        workbook.sheets[0]
            .conditional_formatting
            .push(ConditionalFormattingRule {
                range: ConditionalFormattingRange {
                    start_row: 1,
                    end_row: 10,
                    start_col: 1,
                    end_col: 2,
                },
                rule_type: "greaterThan".to_string(),
                value: Some("10".to_string()),
                value2: None,
                style: Some(ConditionalFormattingStyle {
                    font_color: Some("#006100".to_string()),
                    bg_color: Some("#C6EFCE".to_string()),
                    bold: Some(true),
                    italic: None,
                }),
                scale_colors: Vec::new(),
            });
        let bytes =
            export_workbook_to_xlsx(&workbook).expect("export xlsx with conditional format");
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).expect("read xlsx zip");
        let mut sheet_xml = String::new();
        archive
            .by_name("xl/worksheets/sheet1.xml")
            .expect("sheet xml")
            .read_to_string(&mut sheet_xml)
            .expect("read sheet xml");
        assert!(sheet_xml.contains("conditionalFormatting"));
        assert!(sheet_xml.contains("greaterThan"));
        let mut styles_xml = String::new();
        archive
            .by_name("xl/styles.xml")
            .expect("styles xml")
            .read_to_string(&mut styles_xml)
            .expect("read styles xml");
        assert!(styles_xml.contains("006100"));
    }

    #[test]
    fn xlsx_imports_chart_anchor_and_metadata() {
        let mut workbook = WorkbookModel::new_default();
        workbook.sheets[0].cells.insert(
            "1:1".to_string(),
            SheetCell {
                raw_value: "A".to_string(),
                display_value: "A".to_string(),
                formula: None,
                style: None,
            },
        );
        workbook.sheets[0].cells.insert(
            "1:2".to_string(),
            SheetCell {
                raw_value: "3".to_string(),
                display_value: "3".to_string(),
                formula: None,
                style: None,
            },
        );
        workbook.sheets[0].charts.push(ChartModel {
            chart_type: "bar".to_string(),
            title: None,
            start_row: 1,
            end_row: 1,
            start_col: 1,
            end_col: 2,
        });
        let path = std::env::temp_dir().join(format!(
            "redoc-xlsx-charts-{}-{}.xlsx",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::write(
            &path,
            export_workbook_to_xlsx(&workbook).expect("export xlsx"),
        )
        .expect("write xlsx");
        let imported = import_workbook_from_xlsx_with_report(&path).expect("import xlsx");
        assert!(
            imported
                .warnings
                .iter()
                .all(|warning| !warning.to_lowercase().contains("skipped")),
            "chart import unexpectedly warned: {:?}",
            imported.warnings
        );
        assert_eq!(imported.workbook.sheets[0].charts.len(), 1);
        let chart = &imported.workbook.sheets[0].charts[0];
        assert_eq!(chart.chart_type, "bar");
        assert_eq!(chart.start_row, 3);
        assert_eq!(chart.end_row, 17);
        assert_eq!(chart.start_col, 1);
        assert_eq!(chart.end_col, 8);
        std::fs::remove_file(path).expect("cleanup xlsx");
    }

    #[test]
    fn xlsx_round_trip_imports_conditional_format_rules() {
        let mut workbook = WorkbookModel::new_default();
        workbook.sheets[0]
            .conditional_formatting
            .push(ConditionalFormattingRule {
                range: ConditionalFormattingRange {
                    start_row: 2,
                    end_row: 5,
                    start_col: 2,
                    end_col: 3,
                },
                rule_type: "textContains".to_string(),
                value: Some("urgent".to_string()),
                value2: None,
                style: Some(ConditionalFormattingStyle {
                    font_color: Some("#9C0006".to_string()),
                    bg_color: Some("#FFC7CE".to_string()),
                    bold: Some(true),
                    italic: Some(true),
                }),
                scale_colors: Vec::new(),
            });
        let path = std::env::temp_dir().join(format!(
            "redoc-xlsx-cf-{}-{}.xlsx",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::write(
            &path,
            export_workbook_to_xlsx(&workbook).expect("export xlsx"),
        )
        .expect("write xlsx");
        let imported = import_workbook_from_xlsx_with_report(&path).expect("import xlsx");
        let rules = &imported.workbook.sheets[0].conditional_formatting;
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].rule_type, "textContains");
        assert_eq!(rules[0].value.as_deref(), Some("urgent"));
        assert_eq!(rules[0].range.start_row, 2);
        assert_eq!(rules[0].range.end_col, 3);
        assert_eq!(
            rules[0]
                .style
                .as_ref()
                .and_then(|style| style.font_color.as_deref()),
            Some("#9C0006")
        );
        assert_eq!(
            rules[0]
                .style
                .as_ref()
                .and_then(|style| style.bg_color.as_deref()),
            Some("#FFC7CE")
        );
        std::fs::remove_file(path).expect("cleanup xlsx");
    }

    #[test]
    fn xlsx_round_trips_cell_borders() {
        use redoc_sheet_engine::{BorderEdge, CellBorders, CellStyle};
        let mut workbook = WorkbookModel::new_default();
        workbook.sheets[0].cells.insert(
            "1:1".to_string(),
            SheetCell {
                raw_value: "boxed".to_string(),
                display_value: "boxed".to_string(),
                formula: None,
                style: Some(CellStyle {
                    borders: Some(CellBorders {
                        top: Some(BorderEdge { style: "thin".to_string(), color: Some("#000000".to_string()) }),
                        bottom: Some(BorderEdge { style: "medium".to_string(), color: None }),
                        left: Some(BorderEdge { style: "dashed".to_string(), color: Some("#1f2937".to_string()) }),
                        right: Some(BorderEdge { style: "dotted".to_string(), color: None }),
                    }),
                    ..Default::default()
                }),
            },
        );
        let path = std::env::temp_dir().join(format!(
            "redoc-xlsx-borders-{}-{}.xlsx",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::write(
            &path,
            export_workbook_to_xlsx(&workbook).expect("export xlsx"),
        )
        .expect("write xlsx");
        let imported = import_workbook_from_xlsx_with_report(&path).expect("import xlsx");
        let borders = imported
            .workbook
            .sheets[0]
            .cells
            .get("1:1")
            .and_then(|cell| cell.style.as_ref())
            .and_then(|style| style.borders.as_ref())
            .expect("borders survive xlsx round-trip");
        assert_eq!(borders.top.as_ref().unwrap().style, "thin");
        assert_eq!(borders.bottom.as_ref().unwrap().style, "medium");
        assert_eq!(borders.left.as_ref().unwrap().style, "dashed");
        assert_eq!(borders.right.as_ref().unwrap().style, "dotted");
        // Border colors survive where the writer emits them.
        assert_eq!(
            borders.top.as_ref().unwrap().color.as_deref(),
            Some("#000000")
        );
        std::fs::remove_file(path).expect("cleanup xlsx");
    }
}
