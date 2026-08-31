use crate::pdf::ExportError;
use calamine::{open_workbook_auto, Data, Reader};
use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader as XmlReader;
use redoc_sheet_engine::{CellStyle, ChartModel, SheetCell, SheetData, WorkbookModel};
use rust_xlsxwriter::{Chart, ChartType, Color, Format, FormatAlign, Workbook};
use std::collections::{BTreeMap, HashMap};
use std::io::Read;
use std::path::Path;

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
}

#[derive(Default)]
struct ImportedXf {
    num_format_id: u32,
    format_code: Option<String>,
    alignment: Option<String>,
    font_id: usize,
    fill_id: usize,
    bold: Option<bool>,
    italic: Option<bool>,
    underline: Option<bool>,
    font_color: Option<String>,
    bg_color: Option<String>,
}

#[derive(Default)]
struct ImportedFont {
    bold: bool,
    italic: bool,
    underline: bool,
    color: Option<String>,
}

#[derive(Default)]
struct ImportedFill {
    color: Option<String>,
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
        }
        if let Some(fill) = fills.get(style.fill_id) {
            style.bg_color = fill.color.clone();
        }
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
    let path = format!("xl/worksheets/sheet{}.xml", sheet_index + 1);
    let Ok(mut file) = archive.by_name(&path) else {
        return metadata;
    };
    let mut xml = Vec::new();
    if file.read_to_end(&mut xml).is_err() {
        return metadata;
    }
    let mut reader = XmlReader::from_reader(xml.as_slice());
    reader.config_mut().trim_text(true);
    let mut buffer = Vec::new();
    loop {
        match reader.read_event_into(&mut buffer) {
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
        || style.bg_color.is_some())
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

fn color(value: &str) -> Option<Color> {
    let value = value.trim().trim_start_matches('#');
    (value.len() == 6)
        .then(|| u32::from_str_radix(value, 16).ok())
        .flatten()
        .map(Color::RGB)
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
    if let Some(font_color) = style.font_color.as_deref().and_then(color) {
        format = format.set_font_color(font_color);
    }
    if let Some(bg_color) = style.bg_color.as_deref().and_then(color) {
        format = format.set_background_color(bg_color);
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
    if let Some(number_format) = style.format.as_deref() {
        let number_format = match number_format {
            "currency" => "$#,##0.00",
            "percent" => "0.00%",
            "number" => "#,##0.00",
            _ => "General",
        };
        format = format.set_num_format(number_format);
    }
    format
}

fn chart_type_for(model: &ChartModel) -> ChartType {
    match model.chart_type.as_str() {
        "line" => ChartType::Line,
        "pie" => ChartType::Pie,
        _ => ChartType::Bar,
    }
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

                if let Some(ref f) = cell.formula {
                    let _ = ws.write_formula_with_format(row, col, f.as_str(), &format);
                } else if let Ok(n) = cell.raw_value.parse::<f64>() {
                    let _ = ws.write_number_with_format(row, col, n, &format);
                } else {
                    let _ = ws.write_string_with_format(row, col, &cell.raw_value, &format);
                }
            }
        }

        insert_sheet_charts(ws, &sheet_data.name, &sheet_data.charts)?;
    }

    let buf = wb
        .save_to_buffer()
        .map_err(|e| ExportError::Xlsx(format!("{:?}", e)))?;
    Ok(buf)
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

fn xml_local_name(name: &[u8]) -> &[u8] {
    name.rsplit(|byte| *byte == b':').next().unwrap_or(name)
}

fn xml_attr_local(element: &BytesStart<'_>, name: &[u8]) -> Option<String> {
    element.attributes().flatten().find_map(|attribute| {
        (xml_local_name(attribute.key.as_ref()) == name)
            .then(|| String::from_utf8_lossy(attribute.value.as_ref()).into_owned())
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

#[derive(Default)]
struct ChartAnchor {
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

fn parse_drawing_anchors(xml: &[u8]) -> Vec<ChartAnchor> {
    let mut reader = XmlReader::from_reader(xml);
    reader.config_mut().trim_text(true);
    let mut buffer = Vec::new();
    let mut anchor: Option<ChartAnchor> = None;
    let mut section = None;
    let mut capture = None::<Vec<u8>>;
    let mut captured_text = String::new();
    let mut anchors = Vec::new();
    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(element)) => {
                let name = xml_local_name(element.name().as_ref()).to_vec();
                match name.as_slice() {
                    b"twoCellAnchor" | b"oneCellAnchor" => anchor = Some(ChartAnchor::default()),
                    b"from" => section = Some(AnchorSection::From),
                    b"to" => section = Some(AnchorSection::To),
                    b"row" | b"col" if anchor.is_some() => {
                        capture = Some(name);
                        captured_text.clear();
                    }
                    b"chart" if anchor.is_some() => {
                        if let Some(anchor) = anchor.as_mut() {
                            anchor.relationship =
                                xml_attr_local(&element, b"id").unwrap_or_default();
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Empty(element)) => {
                let name = xml_local_name(element.name().as_ref()).to_vec();
                if name.as_slice() == b"chart" {
                    if let Some(anchor) = anchor.as_mut() {
                        anchor.relationship = xml_attr_local(&element, b"id").unwrap_or_default();
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
    let chart_type = if xml
        .windows(b"lineChart".len())
        .any(|window| window == b"lineChart")
    {
        "line"
    } else if xml
        .windows(b"pieChart".len())
        .any(|window| window == b"pieChart")
    {
        "pie"
    } else {
        "bar"
    }
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

pub fn import_workbook_from_xlsx(path: &Path) -> Result<WorkbookModel, ExportError> {
    Ok(import_workbook_from_xlsx_with_report(path)?.workbook)
}

pub fn import_workbook_from_xlsx_with_report(path: &Path) -> Result<XlsxImportResult, ExportError> {
    let mut source =
        open_workbook_auto(path).map_err(|error| ExportError::Xlsx(error.to_string()))?;
    let mut archive = std::fs::read(path)
        .ok()
        .and_then(|bytes| zip::ZipArchive::new(std::io::Cursor::new(bytes)).ok());
    let mut warnings = Vec::new();
    let has_chart_parts = archive.as_mut().map(archive_has_charts).unwrap_or(false);
    let mut sheets = Vec::new();
    for (sheet_index, name) in source.sheet_names().to_owned().into_iter().enumerate() {
        let range = source
            .worksheet_range(&name)
            .map_err(|error| ExportError::Xlsx(error.to_string()))?;
        let formulas = source
            .worksheet_formula(&name)
            .map_err(|error| ExportError::Xlsx(error.to_string()))?;
        let mut sheet = SheetData::new(&format!("sheet-{}", sheet_index + 1), &name);
        for (row, values) in range.rows().enumerate() {
            for (column, value) in values.iter().enumerate() {
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
                    .get_value((row as u32, column as u32))
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
                    format!("{}:{}", row + 1, column + 1),
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
            for (key, cell) in &mut sheet.cells {
                if let Ok((row, column)) = redoc_sheet_engine::parse_key(key) {
                    cell.style = imported_style(&metadata, &excel_reference(row, column));
                }
            }
            sheet.charts = import_sheet_charts(archive, sheet_index, &mut warnings);
        }
        sheets.push(sheet);
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
            recalc_plans: Vec::new(),
            named_ranges: Vec::new(),
            cross_sheet_dependents: std::collections::HashMap::new(),
            cross_sheet_index_valid: false,
        },
        warnings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
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
    fn xlsx_import_warns_when_charts_present() {
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
}
