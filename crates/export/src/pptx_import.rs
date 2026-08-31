use crate::base64_util::encode_base64;
use crate::pdf::ExportError;
use quick_xml::events::Event;
use quick_xml::Reader;
use redoc_slide_engine::{DeckModel, ElementKind, SlideElement};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::io::{Error, ErrorKind, Read};
use std::path::Path;

const EMU_PER_CANVAS_UNIT: f64 = 9_525.0;
const MAX_XML_BYTES: u64 = 16 * 1024 * 1024;
const MAX_MEDIA_BYTES: u64 = 32 * 1024 * 1024;
const MAX_ARCHIVE_ENTRIES: usize = 10_000;
const MAX_TOTAL_MEDIA_BYTES: u64 = 256 * 1024 * 1024;

#[derive(Debug, Clone, Serialize)]
pub struct PptxImportResult {
    pub deck: DeckModel,
    pub warnings: Vec<String>,
}

#[derive(Default)]
struct ElementBuilder {
    kind: BuilderKind,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    rotation: f64,
    shape_type: String,
    text: String,
    font_size: f64,
    font_family: String,
    color: String,
    align: String,
    bold: bool,
    italic: bool,
    underline: bool,
    bullets: bool,
    fill_color: String,
    stroke_color: String,
    stroke_width: f64,
    image_rel: Option<String>,
    shape_id: Option<u32>,
    table_data: Vec<Vec<String>>,
    table_row: Option<Vec<String>>,
    table_cell: Option<String>,
    chart_rel: Option<String>,
}

#[derive(Default, PartialEq, Eq)]
enum BuilderKind {
    #[default]
    TextOrShape,
    Connector,
    Image,
    Table,
    Chart,
    UnsupportedGraphic,
}

fn invalid_data(message: impl Into<String>) -> ExportError {
    ExportError::Io(Error::new(ErrorKind::InvalidData, message.into()))
}

fn local_name(name: &[u8]) -> &[u8] {
    name.rsplit(|byte| *byte == b':').next().unwrap_or(name)
}

fn attribute(event: &quick_xml::events::BytesStart<'_>, name: &[u8]) -> Option<String> {
    event.attributes().flatten().find_map(|attr| {
        (local_name(attr.key.as_ref()) == name)
            .then(|| String::from_utf8_lossy(attr.value.as_ref()).into_owned())
    })
}

fn parse_f64(value: Option<&str>, fallback: f64) -> f64 {
    value
        .and_then(|value| value.parse::<f64>().ok())
        .unwrap_or(fallback)
}

fn parse_emu(value: Option<&str>) -> f64 {
    parse_f64(value, 0.0) / EMU_PER_CANVAS_UNIT
}

fn parse_color(value: &str, fallback: &str) -> String {
    let value = value.trim();
    if value.len() == 6 && value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        format!("#{value}")
    } else {
        fallback.to_string()
    }
}

fn mime_for_path(path: &str) -> &'static str {
    match path
        .rsplit('.')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        _ => "image/png",
    }
}

fn normalize_target(target: &str) -> Option<String> {
    let mut parts = Vec::new();
    if target.starts_with('/') {
        parts.push("ppt".to_string());
    } else {
        parts.extend(["ppt".to_string(), "slides".to_string()]);
    }
    for part in target.trim_start_matches('/').split('/') {
        match part {
            "" | "." => {}
            ".." => {
                if parts.len() <= 1 {
                    return None;
                }
                parts.pop();
            }
            value => parts.push(value.to_string()),
        }
    }
    Some(parts.join("/"))
}

fn read_entry(
    archive: &mut zip::ZipArchive<std::fs::File>,
    path: &str,
    limit: u64,
) -> Result<Vec<u8>, ExportError> {
    let mut entry = archive
        .by_name(path)
        .map_err(|error| invalid_data(format!("missing {path}: {error}")))?;
    if entry.size() > limit {
        return Err(invalid_data(format!(
            "{path} exceeds the {limit} byte limit"
        )));
    }
    let mut bytes = Vec::with_capacity(entry.size() as usize);
    entry.read_to_end(&mut bytes)?;
    Ok(bytes)
}

fn read_relationships(
    archive: &mut zip::ZipArchive<std::fs::File>,
    slide_number: usize,
) -> Result<HashMap<String, String>, ExportError> {
    let path = format!("ppt/slides/_rels/slide{slide_number}.xml.rels");
    let Ok(bytes) = read_entry(archive, &path, MAX_XML_BYTES) else {
        return Ok(HashMap::new());
    };
    let mut reader = Reader::from_reader(bytes.as_slice());
    let mut buffer = Vec::new();
    let mut relationships = HashMap::new();
    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Empty(event)) | Ok(Event::Start(event))
                if local_name(event.name().as_ref()) == b"Relationship" =>
            {
                let id = attribute(&event, b"Id");
                let target = attribute(&event, b"Target");
                if let (Some(id), Some(target)) = (id, target) {
                    if let Some(path) = normalize_target(&target) {
                        relationships.insert(id, path);
                    }
                }
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => return Err(invalid_data(format!("invalid relationships XML: {error}"))),
        }
        buffer.clear();
    }
    Ok(relationships)
}

/// Return shape ids targeted by the supported native fade entrance effect.
///
/// PresentationML timing trees are intentionally parsed independently from the
/// drawing tree.  This keeps malformed/unknown timeline nodes non-fatal while
/// preserving the one animation primitive the editor can represent.
fn animated_shape_ids(xml: &[u8]) -> HashSet<u32> {
    let mut reader = Reader::from_reader(xml);
    let mut buffer = Vec::new();
    let mut fade_effect_depth = 0usize;
    let mut ids = HashSet::new();
    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(event)) => {
                let name = local_name(event.name().as_ref()).to_vec();
                if name.as_slice() == b"animEffect"
                    && attribute(&event, b"transition").as_deref() == Some("in")
                    && attribute(&event, b"filter").as_deref() == Some("fade")
                {
                    fade_effect_depth = fade_effect_depth.saturating_add(1);
                } else if name.as_slice() == b"spTgt" && fade_effect_depth > 0 {
                    if let Some(id) = attribute(&event, b"spid").and_then(|value| value.parse().ok())
                    {
                        ids.insert(id);
                    }
                }
            }
            Ok(Event::Empty(event)) => {
                let name = local_name(event.name().as_ref()).to_vec();
                if name.as_slice() == b"spTgt" && fade_effect_depth > 0 {
                    if let Some(id) = attribute(&event, b"spid").and_then(|value| value.parse().ok())
                    {
                        ids.insert(id);
                    }
                }
            }
            Ok(Event::End(event)) => {
                if local_name(event.name().as_ref()) == b"animEffect" && fade_effect_depth > 0 {
                    fade_effect_depth -= 1;
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            Ok(_) => {}
        }
        buffer.clear();
    }
    ids
}

fn parse_chart_part(xml: &[u8]) -> Option<(String, Vec<f64>, Vec<String>)> {
    let mut reader = Reader::from_reader(xml);
    reader.config_mut().trim_text(true);
    let mut buffer = Vec::new();
    let mut chart_type = None::<String>;
    let mut title = None::<String>;
    let mut labels = Vec::new();
    let mut values = Vec::new();
    let mut in_title = false;
    let mut in_categories = false;
    let mut in_values = false;
    let mut in_value = false;
    let mut current_value = String::new();

    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(event)) => {
                let name = local_name(event.name().as_ref()).to_vec();
                match name.as_slice() {
                    b"barChart" => chart_type = Some("bar".to_string()),
                    b"lineChart" => chart_type = Some("line".to_string()),
                    b"pieChart" => chart_type = Some("pie".to_string()),
                    b"title" => in_title = true,
                    b"cat" => in_categories = true,
                    b"val" => in_values = true,
                    b"v" if in_title || in_categories || in_values => {
                        in_value = true;
                        current_value.clear();
                    }
                    b"t" if in_title => {
                        in_value = true;
                        current_value.clear();
                    }
                    _ => {}
                }
            }
            Ok(Event::Text(event)) if in_value => {
                current_value.push_str(
                    &event
                        .unescape()
                        .ok()
                        .map(|text| text.into_owned())
                        .unwrap_or_default(),
                );
            }
            Ok(Event::End(event)) => {
                let name = local_name(event.name().as_ref()).to_vec();
                match name.as_slice() {
                    b"v" | b"t" if in_value => {
                        if in_title {
                            if title.is_none() {
                                title = Some(current_value.clone());
                            }
                        } else if in_categories {
                            labels.push(current_value.clone());
                        } else if in_values {
                            if let Ok(value) = current_value.parse::<f64>() {
                                values.push(value);
                            }
                        }
                        in_value = false;
                    }
                    b"title" => in_title = false,
                    b"cat" => in_categories = false,
                    b"val" => in_values = false,
                    _ => {}
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            Ok(_) => {}
        }
        buffer.clear();
    }

    let chart_type = chart_type?;
    let mut all_labels = Vec::with_capacity(labels.len() + 1);
    all_labels.push(title.unwrap_or_else(|| "Chart".to_string()));
    all_labels.extend(labels);
    Some((chart_type, values, all_labels))
}

fn parse_slide(
    xml: &[u8],
    slide_number: usize,
    archive: &mut zip::ZipArchive<std::fs::File>,
    relationships: &HashMap<String, String>,
    warnings: &mut Vec<String>,
    media_bytes: &mut u64,
) -> Result<redoc_slide_engine::Slide, ExportError> {
    let mut reader = Reader::from_reader(xml);
    reader.config_mut().trim_text(true);
    let mut buffer = Vec::new();
    let mut stack: Vec<Vec<u8>> = Vec::new();
    let mut current: Option<ElementBuilder> = None;
    let mut in_text = false;
    let mut color_context = None::<&'static str>;
    let mut elements = Vec::new();
    let mut z_index = 0i32;
    let animated_ids = animated_shape_ids(xml);

    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(event)) => {
                let name = local_name(event.name().as_ref()).to_vec();
                match name.as_slice() {
                    b"sp" if current.is_none() => current = Some(ElementBuilder::default()),
                    b"cxnSp" if current.is_none() => {
                        current = Some(ElementBuilder {
                            kind: BuilderKind::Connector,
                            stroke_color: "#1e293b".to_string(),
                            stroke_width: 1.0,
                            ..Default::default()
                        });
                    }
                    b"pic" if current.is_none() => {
                        current = Some(ElementBuilder {
                            kind: BuilderKind::Image,
                            ..Default::default()
                        });
                    }
                    b"graphicFrame" if current.is_none() => {
                        current = Some(ElementBuilder {
                            kind: BuilderKind::UnsupportedGraphic,
                            fill_color: "#fef3c7".to_string(),
                            stroke_color: "#d97706".to_string(),
                            stroke_width: 1.0,
                            ..Default::default()
                        });
                    }
                    b"graphicData" => {
                        if let Some(element) = current.as_mut() {
                            match attribute(&event, b"uri").as_deref() {
                                Some("http://schemas.openxmlformats.org/drawingml/2006/table") => {
                                    element.kind = BuilderKind::Table;
                                }
                                Some("http://schemas.openxmlformats.org/drawingml/2006/chart") => {
                                    element.kind = BuilderKind::Chart;
                                }
                                _ => {}
                            }
                        }
                    }
                    b"chart" => {
                        if let Some(element) = current.as_mut() {
                            if element.kind == BuilderKind::Chart {
                                element.chart_rel = attribute(&event, b"id");
                            }
                        }
                    }
                    b"tbl" => {
                        if let Some(element) = current.as_mut() {
                            if element.kind == BuilderKind::Table {
                                element.table_data.clear();
                            }
                        }
                    }
                    b"tr" => {
                        if let Some(element) = current.as_mut() {
                            if element.kind == BuilderKind::Table {
                                element.table_row = Some(Vec::new());
                            }
                        }
                    }
                    b"tc" => {
                        if let Some(element) = current.as_mut() {
                            if element.kind == BuilderKind::Table {
                                element.table_cell = Some(String::new());
                            }
                        }
                    }
                    b"off" => {
                        if let Some(element) = current.as_mut() {
                            element.x = parse_emu(attribute(&event, b"x").as_deref());
                            element.y = parse_emu(attribute(&event, b"y").as_deref());
                        }
                    }
                    b"cNvPr" => {
                        if let Some(element) = current.as_mut() {
                            element.shape_id = attribute(&event, b"id")
                                .and_then(|value| value.parse::<u32>().ok());
                        }
                    }
                    b"ext" => {
                        if let Some(element) = current.as_mut() {
                            element.width = parse_emu(attribute(&event, b"cx").as_deref());
                            element.height = parse_emu(attribute(&event, b"cy").as_deref());
                        }
                    }
                    b"ln" => {
                        if let Some(element) = current.as_mut() {
                            element.stroke_width =
                                parse_f64(attribute(&event, b"w").as_deref(), 12700.0) / 12700.0;
                        }
                    }
                    b"xfrm" => {
                        if let Some(element) = current.as_mut() {
                            element.rotation =
                                parse_f64(attribute(&event, b"rot").as_deref(), 0.0) / 60_000.0;
                        }
                    }
                    b"prstGeom" => {
                        if let Some(element) = current.as_mut() {
                            element.shape_type =
                                attribute(&event, b"prst").unwrap_or_else(|| "rect".to_string());
                        }
                    }
                    b"blip" => {
                        if let Some(element) = current.as_mut() {
                            element.image_rel = attribute(&event, b"embed");
                        }
                    }
                    b"pPr" => {
                        if let Some(element) = current.as_mut() {
                            element.align = match attribute(&event, b"algn").as_deref() {
                                Some("ctr") => "center",
                                Some("r") => "right",
                                Some("j") => "justify",
                                _ => "left",
                            }
                            .to_string();
                            element.bullets = stack.iter().any(|item| item.as_slice() == b"buChar");
                        }
                    }
                    b"rPr" => {
                        if let Some(element) = current.as_mut() {
                            element.font_size =
                                parse_f64(attribute(&event, b"sz").as_deref(), 1800.0) / 100.0;
                            element.bold = attribute(&event, b"b").as_deref() == Some("1");
                            element.italic = attribute(&event, b"i").as_deref() == Some("1");
                            element.underline =
                                attribute(&event, b"u").is_some_and(|value| value != "none");
                        }
                    }
                    b"latin" => {
                        if let Some(element) = current.as_mut() {
                            element.font_family =
                                attribute(&event, b"typeface").unwrap_or_default();
                        }
                    }
                    b"solidFill" => {
                        color_context = if stack.iter().any(|item| item.as_slice() == b"rPr") {
                            Some("text")
                        } else if stack.iter().any(|item| item.as_slice() == b"ln") {
                            Some("stroke")
                        } else {
                            Some("fill")
                        };
                    }
                    b"srgbClr" => {
                        if let (Some(context), Some(value), Some(element)) =
                            (color_context, attribute(&event, b"val"), current.as_mut())
                        {
                            match context {
                                "text" => element.color = parse_color(&value, "#1e293b"),
                                "stroke" => element.stroke_color = parse_color(&value, "#1e293b"),
                                "fill" => element.fill_color = parse_color(&value, "#ffffff"),
                                _ => {}
                            }
                        }
                    }
                    b"t" => in_text = true,
                    _ => {}
                }
                stack.push(name);
            }
            Ok(Event::Empty(event)) => {
                let name = local_name(event.name().as_ref()).to_vec();
                if name.as_slice() == b"cNvPr" {
                    if let Some(element) = current.as_mut() {
                        element.shape_id = attribute(&event, b"id")
                            .and_then(|value| value.parse::<u32>().ok());
                    }
                } else if name.as_slice() == b"off" {
                    if let Some(element) = current.as_mut() {
                        element.x = parse_emu(attribute(&event, b"x").as_deref());
                        element.y = parse_emu(attribute(&event, b"y").as_deref());
                    }
                } else if name.as_slice() == b"ext" {
                    if let Some(element) = current.as_mut() {
                        element.width = parse_emu(attribute(&event, b"cx").as_deref());
                        element.height = parse_emu(attribute(&event, b"cy").as_deref());
                    }
                } else if name.as_slice() == b"blip" {
                    if let Some(element) = current.as_mut() {
                        element.image_rel = attribute(&event, b"embed");
                    }
                } else if name.as_slice() == b"chart" {
                    if let Some(element) = current.as_mut() {
                        if element.kind == BuilderKind::Chart {
                            element.chart_rel = attribute(&event, b"id");
                        }
                    }
                } else if name.as_slice() == b"br" {
                    if let Some(element) = current.as_mut() {
                        element.text.push('\n');
                    }
                } else if matches!(name.as_slice(), b"buChar" | b"buAutoNum") {
                    if let Some(element) = current.as_mut() {
                        element.bullets = true;
                    }
                } else if name.as_slice() == b"srgbClr" {
                    if let (Some(context), Some(value), Some(element)) =
                        (color_context, attribute(&event, b"val"), current.as_mut())
                    {
                        match context {
                            "text" => element.color = parse_color(&value, "#1e293b"),
                            "stroke" => element.stroke_color = parse_color(&value, "#1e293b"),
                            "fill" => element.fill_color = parse_color(&value, "#ffffff"),
                            _ => {}
                        }
                    }
                }
            }
            Ok(Event::Text(event)) if in_text => {
                if let Some(element) = current.as_mut() {
                    let text = event
                        .unescape()
                        .map_err(|error| invalid_data(format!("invalid slide text: {error}")))?;
                    if element.kind == BuilderKind::Table && element.table_cell.is_some() {
                        if let Some(cell) = element.table_cell.as_mut() {
                            cell.push_str(&text);
                        }
                    } else {
                        element.text.push_str(&text);
                    }
                }
            }
            Ok(Event::End(event)) => {
                let name = local_name(event.name().as_ref()).to_vec();
                if name.as_slice() == b"t" {
                    in_text = false;
                }
                if name.as_slice() == b"tc" {
                    if let Some(element) = current.as_mut() {
                        if element.kind == BuilderKind::Table {
                            if let Some(cell) = element.table_cell.take() {
                                if let Some(row) = element.table_row.as_mut() {
                                    row.push(cell);
                                }
                            }
                        }
                    }
                }
                if name.as_slice() == b"tr" {
                    if let Some(element) = current.as_mut() {
                        if element.kind == BuilderKind::Table {
                            if let Some(row) = element.table_row.take() {
                                element.table_data.push(row);
                            }
                        }
                    }
                }
                if name.as_slice() == b"p" {
                    if let Some(element) = current.as_mut() {
                        if element.kind != BuilderKind::Table && !element.text.ends_with('\n') {
                            element.text.push('\n');
                        }
                    }
                }
                if matches!(name.as_slice(), b"sp" | b"cxnSp" | b"pic" | b"graphicFrame") {
                    if let Some(element) = current.take() {
                        if element.kind == BuilderKind::Image {
                            if let Some(rel_id) = element.image_rel.as_deref() {
                                if let Some(media_path) = relationships.get(rel_id) {
                                    let media_size = archive
                                        .by_name(media_path)
                                        .map(|entry| entry.size())
                                        .unwrap_or(0);
                                    if *media_bytes + media_size > MAX_TOTAL_MEDIA_BYTES {
                                        warnings.push(format!(
                                            "slide {slide_number}: image {media_path} skipped because the total media limit was exceeded"
                                        ));
                                    } else {
                                        match read_entry(archive, media_path, MAX_MEDIA_BYTES) {
                                            Ok(media) => {
                                                *media_bytes += media.len() as u64;
                                                let mime = mime_for_path(media_path);
                                                elements.push(SlideElement {
                                                    id: format!("pptx-{slide_number}-{z_index}"),
                                                    x: element.x,
                                                    y: element.y,
                                                    width: element.width,
                                                    height: element.height,
                                                    rotation: element.rotation,
                                                    z_index,
                                                    entrance: if element
                                                        .shape_id
                                                        .is_some_and(|id| animated_ids.contains(&id))
                                                    {
                                                        "fade".to_string()
                                                    } else {
                                                        "none".to_string()
                                                    },
                                                    kind: ElementKind::Image {
                                                        asset_hash: format!(
                                                            "data:{mime};base64,{}",
                                                            encode_base64(&media)
                                                        ),
                                                        mime: mime.to_string(),
                                                    },
                                                });
                                                z_index += 1;
                                            }
                                            Err(error) => warnings.push(format!(
                                                "slide {slide_number}: image {media_path} skipped ({error})"
                                            )),
                                        }
                                    }
                                } else {
                                    warnings.push(format!("slide {slide_number}: image relationship {rel_id} was missing"));
                                }
                            }
                        } else if element.kind == BuilderKind::Connector {
                            let shape_type = if element.shape_type == "line" {
                                "line"
                            } else {
                                "arrow"
                            };
                            elements.push(SlideElement {
                                id: format!("pptx-{slide_number}-{z_index}"),
                                x: element.x,
                                y: element.y,
                                width: element.width,
                                height: element.height,
                                rotation: element.rotation,
                                z_index,
                                entrance: if element
                                    .shape_id
                                    .is_some_and(|id| animated_ids.contains(&id))
                                {
                                    "fade".to_string()
                                } else {
                                    "none".to_string()
                                },
                                kind: ElementKind::Shape {
                                    shape_type: shape_type.to_string(),
                                    fill_color: element.fill_color,
                                    stroke_color: element.stroke_color,
                                    stroke_width: element.stroke_width.max(0.1),
                                    text: element.text.trim_end_matches('\n').to_string(),
                                },
                            });
                            z_index += 1;
                        } else if element.kind == BuilderKind::Table {
                            let data = element.table_data;
                            let rows = data.len();
                            let cols = data.iter().map(Vec::len).max().unwrap_or(0);
                            elements.push(SlideElement {
                                id: format!("pptx-{slide_number}-{z_index}"),
                                x: element.x,
                                y: element.y,
                                width: element.width,
                                height: element.height,
                                rotation: element.rotation,
                                z_index,
                                entrance: if element
                                    .shape_id
                                    .is_some_and(|id| animated_ids.contains(&id))
                                {
                                    "fade".to_string()
                                } else {
                                    "none".to_string()
                                },
                                kind: ElementKind::Table { rows, cols, data },
                            });
                            z_index += 1;
                        } else if element.kind == BuilderKind::Chart {
                            let chart_path = element
                                .chart_rel
                                .as_deref()
                                .and_then(|rel_id| relationships.get(rel_id))
                                .cloned();
                            let chart = chart_path
                                .as_deref()
                                .and_then(|path| read_entry(archive, path, MAX_XML_BYTES).ok())
                                .and_then(|xml| parse_chart_part(&xml));
                            if let Some((chart_type, data, labels)) = chart {
                                elements.push(SlideElement {
                                    id: format!("pptx-{slide_number}-{z_index}"),
                                    x: element.x,
                                    y: element.y,
                                    width: element.width,
                                    height: element.height,
                                    rotation: element.rotation,
                                    z_index,
                                    entrance: if element
                                        .shape_id
                                        .is_some_and(|id| animated_ids.contains(&id))
                                    {
                                        "fade".to_string()
                                    } else {
                                        "none".to_string()
                                    },
                                    kind: ElementKind::Chart {
                                        chart_type,
                                        data,
                                        labels,
                                    },
                                });
                            } else {
                                warnings.push(format!(
                                    "slide {slide_number}: chart part was missing or unsupported; preserved as a placeholder"
                                ));
                                elements.push(SlideElement {
                                    id: format!("pptx-{slide_number}-{z_index}"),
                                    x: element.x,
                                    y: element.y,
                                    width: element.width.max(120.0),
                                    height: element.height.max(80.0),
                                    rotation: element.rotation,
                                    z_index,
                                    entrance: if element
                                        .shape_id
                                        .is_some_and(|id| animated_ids.contains(&id))
                                    {
                                        "fade".to_string()
                                    } else {
                                        "none".to_string()
                                    },
                                    kind: ElementKind::Shape {
                                        shape_type: "rect".to_string(),
                                        fill_color: "#fef3c7".to_string(),
                                        stroke_color: "#d97706".to_string(),
                                        stroke_width: 1.0,
                                        text: "[Unsupported chart]".to_string(),
                                    },
                                });
                            }
                            z_index += 1;
                        } else if element.kind == BuilderKind::UnsupportedGraphic {
                            warnings.push(format!(
                                "slide {slide_number}: unsupported table or chart preserved as a placeholder"
                            ));
                            let text = element.text.trim().to_string();
                            elements.push(SlideElement {
                                id: format!("pptx-{slide_number}-{z_index}"),
                                x: element.x,
                                y: element.y,
                                width: element.width.max(120.0),
                                height: element.height.max(80.0),
                                rotation: element.rotation,
                                z_index,
                                entrance: if element
                                    .shape_id
                                    .is_some_and(|id| animated_ids.contains(&id))
                                {
                                    "fade".to_string()
                                } else {
                                    "none".to_string()
                                },
                                kind: ElementKind::Shape {
                                    shape_type: "rect".to_string(),
                                    fill_color: element.fill_color,
                                    stroke_color: element.stroke_color,
                                    stroke_width: element.stroke_width.max(0.1),
                                    text: if text.is_empty() {
                                        "[Unsupported table or chart]".to_string()
                                    } else {
                                        format!("[Unsupported graphic: {text}]")
                                    },
                                },
                            });
                            z_index += 1;
                        } else {
                            let text = element.text.trim_end_matches('\n').to_string();
                            if text.trim().is_empty() && element.shape_type.is_empty() {
                                stack.pop();
                                color_context = None;
                                buffer.clear();
                                continue;
                            }
                            if element.shape_type.is_empty() {
                                elements.push(SlideElement {
                                    id: format!("pptx-{slide_number}-{z_index}"),
                                    x: element.x,
                                    y: element.y,
                                    width: element.width,
                                    height: element.height,
                                    rotation: element.rotation,
                                    z_index,
                                    entrance: if element
                                        .shape_id
                                        .is_some_and(|id| animated_ids.contains(&id))
                                    {
                                        "fade".to_string()
                                    } else {
                                        "none".to_string()
                                    },
                                    kind: ElementKind::Text {
                                        text,
                                        font_size: element.font_size.max(1.0),
                                        font_family: if element.font_family.is_empty() {
                                            "Calibri".to_string()
                                        } else {
                                            element.font_family
                                        },
                                        color: if element.color.is_empty() {
                                            "#1e293b".to_string()
                                        } else {
                                            element.color
                                        },
                                        align: if element.align.is_empty() {
                                            "left".to_string()
                                        } else {
                                            element.align
                                        },
                                        bold: element.bold,
                                        italic: element.italic,
                                        underline: element.underline,
                                        bullets: element.bullets,
                                    },
                                });
                            } else {
                                elements.push(SlideElement {
                                    id: format!("pptx-{slide_number}-{z_index}"),
                                    x: element.x,
                                    y: element.y,
                                    width: element.width,
                                    height: element.height,
                                    rotation: element.rotation,
                                    z_index,
                                    entrance: if element
                                        .shape_id
                                        .is_some_and(|id| animated_ids.contains(&id))
                                    {
                                        "fade".to_string()
                                    } else {
                                        "none".to_string()
                                    },
                                    kind: ElementKind::Shape {
                                        shape_type: element.shape_type,
                                        fill_color: if element.fill_color.is_empty() {
                                            "#ffffff".to_string()
                                        } else {
                                            element.fill_color
                                        },
                                        stroke_color: if element.stroke_color.is_empty() {
                                            "#1e293b".to_string()
                                        } else {
                                            element.stroke_color
                                        },
                                        stroke_width: element.stroke_width.max(0.1),
                                        text,
                                    },
                                });
                            }
                            z_index += 1;
                        }
                    }
                }
                stack.pop();
                color_context = None;
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => return Err(invalid_data(format!("invalid slide XML: {error}"))),
        }
        buffer.clear();
    }

    let transition = if xml
        .windows(b"<p:fade".len())
        .any(|window| window == b"<p:fade")
    {
        "fade"
    } else if xml
        .windows(b"<p:push dir=\"l\"".len())
        .any(|window| window == b"<p:push dir=\"l\"")
    {
        "slide-left"
    } else if xml
        .windows(b"<p:push dir=\"r\"".len())
        .any(|window| window == b"<p:push dir=\"r\"")
    {
        "slide-right"
    } else {
        "none"
    };

    Ok(redoc_slide_engine::Slide {
        id: format!("pptx-slide-{slide_number}"),
        layout: "blank".to_string(),
        elements,
        notes: String::new(),
        bg_override: parse_slide_background(xml),
        transition: transition.to_string(),
    })
}

fn parse_notes(xml: &[u8]) -> Result<String, ExportError> {
    let mut reader = Reader::from_reader(xml);
    reader.config_mut().trim_text(true);
    let mut buffer = Vec::new();
    let mut in_text = false;
    let mut notes = String::new();
    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(event)) => in_text = local_name(event.name().as_ref()) == b"t",
            Ok(Event::Text(event)) if in_text => {
                notes.push_str(
                    &event
                        .unescape()
                        .map_err(|error| invalid_data(format!("invalid notes XML: {error}")))?,
                );
            }
            Ok(Event::End(event)) if local_name(event.name().as_ref()) == b"t" => in_text = false,
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => return Err(invalid_data(format!("invalid notes XML: {error}"))),
        }
        buffer.clear();
    }
    Ok(notes)
}

fn parse_canvas_size(xml: &[u8]) -> (f64, f64) {
    let mut reader = Reader::from_reader(xml);
    let mut buffer = Vec::new();
    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Empty(event)) | Ok(Event::Start(event))
                if local_name(event.name().as_ref()) == b"sldSz" =>
            {
                return (
                    parse_emu(attribute(&event, b"cx").as_deref()).max(1.0),
                    parse_emu(attribute(&event, b"cy").as_deref()).max(1.0),
                );
            }
            Ok(Event::Eof) | Err(_) => break,
            Ok(_) => {}
        }
        buffer.clear();
    }
    (960.0, 540.0)
}

fn parse_slide_background(xml: &[u8]) -> Option<String> {
    let mut reader = Reader::from_reader(xml);
    reader.config_mut().trim_text(true);
    let mut buffer = Vec::new();
    let mut in_background = false;
    let mut background_depth = 0usize;
    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(event)) => {
                let name = local_name(event.name().as_ref()).to_vec();
                if name.as_slice() == b"bg" {
                    in_background = true;
                    background_depth = 1;
                } else if in_background {
                    background_depth = background_depth.saturating_add(1);
                    if name.as_slice() == b"srgbClr" {
                        if let Some(value) = attribute(&event, b"val") {
                            let value = value.trim();
                            if value.len() == 6
                                && value.bytes().all(|byte| byte.is_ascii_hexdigit())
                            {
                                return Some(format!("#{value}"));
                            }
                        }
                    }
                }
            }
            Ok(Event::Empty(event)) => {
                if in_background && local_name(event.name().as_ref()) == b"srgbClr" {
                    if let Some(value) = attribute(&event, b"val") {
                        let value = value.trim();
                        if value.len() == 6 && value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                            return Some(format!("#{value}"));
                        }
                    }
                }
            }
            Ok(Event::End(event)) => {
                if in_background {
                    background_depth = background_depth.saturating_sub(1);
                    if local_name(event.name().as_ref()) == b"bg" || background_depth == 0 {
                        in_background = false;
                    }
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            Ok(_) => {}
        }
        buffer.clear();
    }
    None
}

fn normalize_presentation_target(target: &str) -> Option<String> {
    let target = target.trim();
    let mut parts = if target.starts_with('/') {
        Vec::new()
    } else {
        vec!["ppt".to_string()]
    };
    for part in target.trim_start_matches('/').split('/') {
        match part {
            "" | "." => {}
            ".." => {
                if parts.is_empty() {
                    return None;
                }
                parts.pop();
            }
            value => parts.push(value.to_string()),
        }
    }
    if parts.first().is_none_or(|part| part != "ppt") {
        parts.insert(0, "ppt".to_string());
    }
    Some(parts.join("/"))
}

fn parse_presentation_slide_order(presentation_xml: &[u8], relationships_xml: &[u8]) -> Vec<usize> {
    let mut relationships = HashMap::new();
    let mut rel_reader = Reader::from_reader(relationships_xml);
    rel_reader.config_mut().trim_text(true);
    let mut buffer = Vec::new();
    loop {
        match rel_reader.read_event_into(&mut buffer) {
            Ok(Event::Empty(event)) | Ok(Event::Start(event))
                if local_name(event.name().as_ref()) == b"Relationship" =>
            {
                if let (Some(id), Some(target)) =
                    (attribute(&event, b"Id"), attribute(&event, b"Target"))
                {
                    if let Some(path) = normalize_presentation_target(&target) {
                        relationships.insert(id, path);
                    }
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            Ok(_) => {}
        }
        buffer.clear();
    }

    let mut order = Vec::new();
    let mut reader = Reader::from_reader(presentation_xml);
    reader.config_mut().trim_text(true);
    buffer.clear();
    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Empty(event)) | Ok(Event::Start(event))
                if local_name(event.name().as_ref()) == b"sldId" =>
            {
                let Some(rel_id) = event.attributes().flatten().find_map(|attr| {
                    let key = attr.key.as_ref();
                    (key == b"r:id" || key.ends_with(b":id"))
                        .then(|| String::from_utf8_lossy(attr.value.as_ref()).into_owned())
                }) else {
                    buffer.clear();
                    continue;
                };
                let Some(path) = relationships.get(&rel_id) else {
                    buffer.clear();
                    continue;
                };
                if let Some(number) = path
                    .strip_prefix("ppt/slides/slide")
                    .and_then(|value| value.strip_suffix(".xml"))
                    .and_then(|value| value.parse::<usize>().ok())
                {
                    order.push(number);
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            Ok(_) => {}
        }
        buffer.clear();
    }
    order
}

fn parse_theme(xml: &[u8]) -> redoc_slide_engine::SlideTheme {
    let mut theme = redoc_slide_engine::SlideTheme::default();
    let mut reader = Reader::from_reader(xml);
    reader.config_mut().trim_text(true);
    let mut buffer = Vec::new();
    let mut color_slot = None::<&'static str>;
    let mut in_major_font = false;

    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(event)) => {
                let name = local_name(event.name().as_ref()).to_vec();
                match name.as_slice() {
                    b"theme" => {
                        if let Some(value) = attribute(&event, b"name") {
                            theme.name = value;
                        }
                    }
                    b"dk1" => color_slot = Some("text"),
                    b"lt1" => color_slot = Some("background"),
                    b"accent1" => color_slot = Some("accent"),
                    b"majorFont" => in_major_font = true,
                    b"latin" if in_major_font => {
                        if let Some(value) = attribute(&event, b"typeface") {
                            if !value.trim().is_empty() {
                                theme.font_family = value;
                            }
                        }
                    }
                    b"srgbClr" => {
                        if let (Some(slot), Some(value)) =
                            (color_slot, attribute(&event, b"val"))
                        {
                            let value = value.trim();
                            if value.len() == 6 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
                            {
                                let color = format!("#{value}");
                                match slot {
                                    "text" => theme.text_color = color,
                                    "background" => theme.bg_color = color,
                                    "accent" => theme.accent_color = color,
                                    _ => {}
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Empty(event)) => {
                let name = local_name(event.name().as_ref()).to_vec();
                if name.as_slice() == b"srgbClr" {
                    if let (Some(slot), Some(value)) =
                        (color_slot, attribute(&event, b"val"))
                    {
                        let value = value.trim();
                        if value.len() == 6 && value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                            let color = format!("#{value}");
                            match slot {
                                "text" => theme.text_color = color,
                                "background" => theme.bg_color = color,
                                "accent" => theme.accent_color = color,
                                _ => {}
                            }
                        }
                    }
                } else if name.as_slice() == b"latin" && in_major_font {
                    if let Some(value) = attribute(&event, b"typeface") {
                        if !value.trim().is_empty() {
                            theme.font_family = value;
                        }
                    }
                }
            }
            Ok(Event::End(event)) => match local_name(event.name().as_ref()) {
                b"dk1" | b"lt1" | b"accent1" => color_slot = None,
                b"majorFont" => in_major_font = false,
                _ => {}
            },
            Ok(Event::Eof) | Err(_) => break,
            Ok(_) => {}
        }
        buffer.clear();
    }

    theme.id = "pptx-imported-theme".to_string();
    theme
}

pub fn import_deck_from_pptx_with_report(path: &Path) -> Result<PptxImportResult, ExportError> {
    let file = std::fs::File::open(path)?;
    let mut archive = zip::ZipArchive::new(file)?;
    if archive.len() > MAX_ARCHIVE_ENTRIES {
        return Err(invalid_data(format!(
            "PPTX archive has too many entries (maximum {MAX_ARCHIVE_ENTRIES})"
        )));
    }
    let presentation = read_entry(&mut archive, "ppt/presentation.xml", MAX_XML_BYTES)?;
    let (canvas_width, canvas_height) = parse_canvas_size(&presentation);
    let theme = read_entry(&mut archive, "ppt/theme/theme1.xml", MAX_XML_BYTES)
        .map(|xml| parse_theme(&xml))
        .unwrap_or_default();
    let relationship_xml = read_entry(
        &mut archive,
        "ppt/_rels/presentation.xml.rels",
        MAX_XML_BYTES,
    )
    .ok();
    let mut slide_numbers = relationship_xml
        .as_deref()
        .map(|rels| parse_presentation_slide_order(&presentation, rels))
        .unwrap_or_default();
    let mut discovered_slides = Vec::new();
    for index in 0..archive.len() {
        let name = archive.by_index(index)?.name().to_string();
        if let Some(number) = name
            .strip_prefix("ppt/slides/slide")
            .and_then(|value| value.strip_suffix(".xml"))
            .and_then(|value| value.parse::<usize>().ok())
        {
            discovered_slides.push(number);
        }
    }
    discovered_slides.sort_unstable();
    if slide_numbers.is_empty() {
        slide_numbers = discovered_slides;
    } else {
        for number in discovered_slides {
            if !slide_numbers.contains(&number) {
                slide_numbers.push(number);
            }
        }
    }
    slide_numbers.dedup();
    if slide_numbers.is_empty() {
        return Err(invalid_data("PPTX contains no slides"));
    }

    let mut warnings = Vec::new();
    let mut slides = Vec::with_capacity(slide_numbers.len());
    let mut media_bytes = 0u64;
    for slide_number in slide_numbers {
        let slide_path = format!("ppt/slides/slide{slide_number}.xml");
        let xml = read_entry(&mut archive, &slide_path, MAX_XML_BYTES)?;
        let relationships = read_relationships(&mut archive, slide_number)?;
        let mut slide = parse_slide(
            &xml,
            slide_number,
            &mut archive,
            &relationships,
            &mut warnings,
            &mut media_bytes,
        )?;
        let notes_path = format!("ppt/notesSlides/notesSlide{slide_number}.xml");
        if let Ok(notes_xml) = read_entry(&mut archive, &notes_path, MAX_XML_BYTES) {
            slide.notes = parse_notes(&notes_xml)?;
        }
        slides.push(slide);
    }

    let mut deck = DeckModel::new_default();
    deck.slides = slides;
    deck.canvas_width = canvas_width;
    deck.canvas_height = canvas_height;
    deck.theme = theme;
    deck.active_slide_index = 0;
    deck.fade_between_slides = deck.slides.iter().any(|slide| slide.transition == "fade");
    Ok(PptxImportResult { deck, warnings })
}

#[cfg(test)]
mod tests {
    use super::*;
    use redoc_slide_engine::ElementKind;

    #[test]
    fn imports_redoc_pptx_text_shapes_and_notes() {
        let mut source = DeckModel::new_default();
        source.slides[0].elements.clear();
        source.slides[0].notes = "Speaker note".to_string();
        source.slides[0].transition = "fade".to_string();
        source.slides[0].elements.push(SlideElement {
            id: "text".to_string(),
            x: 100.0,
            y: 80.0,
            width: 400.0,
            height: 80.0,
            rotation: 0.0,
            z_index: 1,
            entrance: "none".to_string(),
            kind: ElementKind::Text {
                text: "Imported title".to_string(),
                font_size: 32.0,
                font_family: "Arial".to_string(),
                color: "#ff0000".to_string(),
                align: "center".to_string(),
                bold: true,
                italic: false,
                underline: false,
                bullets: false,
            },
        });
        source.slides[0].elements[0].entrance = "fade".to_string();
        source.slides[0].elements.push(SlideElement {
            id: "shape".to_string(),
            x: 40.0,
            y: 180.0,
            width: 120.0,
            height: 80.0,
            rotation: 15.0,
            z_index: 2,
            entrance: "none".to_string(),
            kind: ElementKind::Shape {
                shape_type: "ellipse".to_string(),
                fill_color: "#00ff00".to_string(),
                stroke_color: "#0000ff".to_string(),
                stroke_width: 2.0,
                text: String::new(),
            },
        });
        source.slides[0].elements.push(SlideElement {
            id: "image".to_string(),
            x: 220.0,
            y: 180.0,
            width: 100.0,
            height: 100.0,
            rotation: 0.0,
            z_index: 3,
            entrance: "none".to_string(),
            kind: ElementKind::Image {
                asset_hash: "data:image/png;base64,aGVsbG8=".to_string(),
                mime: "image/png".to_string(),
            },
        });
        let bytes = crate::pptx::export_deck_to_pptx(&source).expect("export source deck");
        let path =
            std::env::temp_dir().join(format!("redoc-pptx-import-{}.pptx", std::process::id()));
        std::fs::write(&path, bytes).expect("write source deck");
        let result = import_deck_from_pptx_with_report(&path).expect("import source deck");
        let _ = std::fs::remove_file(&path);

        assert_eq!(result.deck.slides.len(), 1);
        assert_eq!(result.deck.slides[0].notes, "Speaker note");
        assert_eq!(result.deck.slides[0].transition, "fade");
        assert!(result.deck.slides[0]
            .elements
            .iter()
            .any(|element| element.entrance == "fade"));
        assert!(result.deck.slides[0]
            .elements
            .iter()
            .any(|element| matches!(
                &element.kind,
                ElementKind::Text { text, bold, .. } if text == "Imported title" && *bold
            )));
        assert!(result.deck.slides[0].elements.iter().any(|element| matches!(
            &element.kind,
            ElementKind::Shape { shape_type, text, .. } if shape_type == "ellipse" && text.is_empty()
        )));
        assert!(result.deck.slides[0].elements.iter().any(|element| matches!(
            &element.kind,
            ElementKind::Image { asset_hash, mime } if asset_hash.starts_with("data:image/png;base64,") && mime == "image/png"
        )));
    }

    #[test]
    fn round_trips_native_pptx_tables() {
        let mut source = DeckModel::new_default();
        source.slides[0].elements.clear();
        source.slides[0].elements.push(SlideElement {
            id: "table".to_string(),
            x: 40.0,
            y: 80.0,
            width: 360.0,
            height: 120.0,
            rotation: 0.0,
            z_index: 1,
            entrance: "none".to_string(),
            kind: ElementKind::Table {
                rows: 2,
                cols: 2,
                data: vec![
                    vec!["Name".to_string(), "Value".to_string()],
                    vec!["A & B".to_string(), "42".to_string()],
                ],
            },
        });

        let bytes = crate::pptx::export_deck_to_pptx(&source).expect("export table deck");
        let slide_xml = {
            let reader = std::io::Cursor::new(bytes.clone());
            let mut archive = zip::ZipArchive::new(reader).expect("read exported archive");
            let mut slide = String::new();
            archive
                .by_name("ppt/slides/slide1.xml")
                .expect("slide part")
                .read_to_string(&mut slide)
                .expect("read slide part");
            slide
        };
        assert!(slide_xml.contains("graphicFrame"));
        assert!(slide_xml.contains("drawingml/2006/table"));
        assert!(slide_xml.contains("A &amp; B"));

        let path = std::env::temp_dir().join(format!(
            "redoc-pptx-table-roundtrip-{}.pptx",
            std::process::id()
        ));
        std::fs::write(&path, bytes).expect("write table deck");
        let result = import_deck_from_pptx_with_report(&path).expect("import table deck");
        let _ = std::fs::remove_file(&path);

        let imported = result.deck.slides[0]
            .elements
            .iter()
            .find_map(|element| match &element.kind {
                ElementKind::Table { rows, cols, data } => Some((*rows, *cols, data.clone())),
                _ => None,
            })
            .expect("native table element");
        assert_eq!(imported.0, 2);
        assert_eq!(imported.1, 2);
        assert_eq!(imported.2[0], vec!["Name", "Value"]);
        assert_eq!(imported.2[1], vec!["A & B", "42"]);
        assert!(!result
            .warnings
            .iter()
            .any(|warning| warning.contains("unsupported table")));
    }

    #[test]
    fn round_trips_native_pptx_charts() {
        let mut source = DeckModel::new_default();
        source.theme.name = "Imported Theme".to_string();
        source.theme.bg_color = "#102030".to_string();
        source.theme.text_color = "#f0f0f0".to_string();
        source.theme.accent_color = "#c06020".to_string();
        source.theme.font_family = "Aptos".to_string();
        source.slides[0].bg_override = Some("#334455".to_string());
        source.slides[0].elements.clear();
        source.slides[0].elements.push(SlideElement {
            id: "chart".to_string(),
            x: 40.0,
            y: 80.0,
            width: 360.0,
            height: 180.0,
            rotation: 0.0,
            z_index: 1,
            entrance: "none".to_string(),
            kind: ElementKind::Chart {
                chart_type: "pie".to_string(),
                data: vec![1.0, 2.0, 3.5],
                labels: vec![
                    "Mix".to_string(),
                    "A".to_string(),
                    "B".to_string(),
                    "C".to_string(),
                ],
            },
        });
        let bytes = crate::pptx::export_deck_to_pptx(&source).expect("export chart deck");
        let path = std::env::temp_dir().join(format!(
            "redoc-pptx-chart-roundtrip-{}.pptx",
            std::process::id()
        ));
        std::fs::write(&path, bytes).expect("write chart deck");
        let result = import_deck_from_pptx_with_report(&path).expect("import chart deck");
        let _ = std::fs::remove_file(&path);

        let imported = result.deck.slides[0]
            .elements
            .iter()
            .find_map(|element| match &element.kind {
                ElementKind::Chart {
                    chart_type,
                    data,
                    labels,
                } => Some((chart_type.clone(), data.clone(), labels.clone())),
                _ => None,
            })
            .expect("native chart element");
        assert_eq!(imported.0, "pie");
        assert_eq!(imported.1, vec![1.0, 2.0, 3.5]);
        assert_eq!(imported.2, vec!["Mix", "A", "B", "C"]);
        assert_eq!(result.deck.theme.name, "Imported Theme");
        assert_eq!(result.deck.theme.bg_color, "#102030");
        assert_eq!(result.deck.theme.text_color, "#f0f0f0");
        assert_eq!(result.deck.theme.accent_color, "#c06020");
        assert_eq!(result.deck.theme.font_family, "Aptos");
        assert_eq!(result.deck.slides[0].bg_override.as_deref(), Some("#334455"));
        assert!(!result
            .warnings
            .iter()
            .any(|warning| warning.contains("chart part was missing")));
    }

    #[test]
    fn honors_presentation_slide_relationship_order() {
        let presentation = br#"<p:presentation xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><p:sldIdLst><p:sldId id="256" r:id="rId3"/><p:sldId id="257" r:id="rId2"/></p:sldIdLst></p:presentation>"#;
        let relationships = br#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide" Target="slides/slide1.xml"/><Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide" Target="slides/slide2.xml"/></Relationships>"#;
        assert_eq!(
            parse_presentation_slide_order(presentation, relationships),
            vec![2, 1]
        );
    }

    #[test]
    fn preserves_unsupported_graphic_frames_as_placeholders() {
        let path = std::env::temp_dir().join(format!(
            "redoc-pptx-placeholder-{}.pptx",
            std::process::id()
        ));
        let file = std::fs::File::create(&path).expect("create empty archive");
        let mut zip = zip::ZipWriter::new(file);
        zip.finish().expect("finish empty archive");
        let file = std::fs::File::open(&path).expect("open empty archive");
        let mut archive = zip::ZipArchive::new(file).expect("read empty archive");
        let mut warnings = Vec::new();
        let mut media_bytes = 0;
        let slide = parse_slide(
            br#"<p:sld xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><p:graphicFrame><p:xfrm><a:off x="95250" y="190500"/><a:ext cx="381000" cy="190500"/></p:xfrm><a:graphic><a:graphicData><a:t>Imported table</a:t></a:graphicData></a:graphic></p:graphicFrame></p:sld>"#,
            1,
            &mut archive,
            &HashMap::new(),
            &mut warnings,
            &mut media_bytes,
        )
        .expect("parse slide");
        let _ = std::fs::remove_file(path);

        assert!(warnings
            .iter()
            .any(|warning| warning.contains("preserved as a placeholder")));
        assert!(slide.elements.iter().any(|element| matches!(
            &element.kind,
            ElementKind::Shape { text, .. } if text == "[Unsupported graphic: Imported table]"
        )));
    }

    #[test]
    fn rejects_pptx_archives_with_too_many_entries() {
        let path = std::env::temp_dir().join(format!(
            "redoc-pptx-entry-limit-{}.pptx",
            std::process::id()
        ));
        let file = std::fs::File::create(&path).expect("create oversized archive");
        let mut zip = zip::ZipWriter::new(file);
        let options = zip::write::FileOptions::default();
        for index in 0..=MAX_ARCHIVE_ENTRIES {
            zip.start_file(format!("padding/{index}"), options)
                .expect("write padding entry");
        }
        zip.finish().expect("finish oversized archive");

        let error = import_deck_from_pptx_with_report(&path).expect_err("entry limit must fail");
        let _ = std::fs::remove_file(path);
        assert!(error.to_string().contains("too many entries"));
    }
}
