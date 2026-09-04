use crate::base64_util::encode_base64;
use crate::pdf::ExportError;
use quick_xml::events::Event;
use quick_xml::Reader;
use redoc_slide_engine::{DeckModel, ElementKind, SlideElement};
use serde::Serialize;
use std::collections::HashMap;
use std::io::{Error, ErrorKind, Read};
use std::path::Path;

const EMU_PER_CANVAS_UNIT: f64 = 9_525.0;
const MAX_XML_BYTES: u64 = 16 * 1024 * 1024;
const MAX_MEDIA_BYTES: u64 = 32 * 1024 * 1024;
const MAX_ARCHIVE_ENTRIES: usize = 10_000;
const MAX_TOTAL_MEDIA_BYTES: u64 = 256 * 1024 * 1024;
const MAX_HYPERLINK_CHARS: usize = 2_048;

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
    hyperlink: Option<String>,
    placeholder_type: Option<String>,
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

/// Convert the bounded DrawingML preset subset into the editor's canonical
/// shape names. Unknown presets are intentionally not retained as arbitrary
/// strings because the editor cannot render their geometry faithfully.
fn canonical_shape_type(preset: &str) -> Option<&'static str> {
    match preset.to_ascii_lowercase().as_str() {
        "rect" => Some("rect"),
        "roundrect" => Some("roundedRect"),
        "ellipse" => Some("ellipse"),
        "triangle" => Some("triangle"),
        "diamond" => Some("diamond"),
        "star" | "star5" => Some("star"),
        "line" => Some("line"),
        _ => None,
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

fn canonical_layout_name(layout_type: &str) -> &'static str {
    match layout_type.trim().to_ascii_lowercase().as_str() {
        "title" => "title",
        "obj" | "tx" | "titleonly" | "objtx" => "title_body",
        "sechead" => "section",
        "twoobj" => "two_col",
        "pic" => "image_caption",
        _ => "blank",
    }
}

fn parse_slide_layout_name(
    archive: &mut zip::ZipArchive<std::fs::File>,
    relationships: &HashMap<String, String>,
) -> String {
    let Some(layout_path) = relationships
        .values()
        .find(|path| path.starts_with("ppt/slideLayouts/"))
        .cloned()
    else {
        return "blank".to_string();
    };
    let Ok(xml) = read_entry(archive, &layout_path, MAX_XML_BYTES) else {
        return "blank".to_string();
    };
    let mut reader = Reader::from_reader(xml.as_slice());
    let mut buffer = Vec::new();
    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(event)) | Ok(Event::Empty(event))
                if local_name(event.name().as_ref()) == b"sldLayout" =>
            {
                return canonical_layout_name(
                    &attribute(&event, b"type").unwrap_or_else(|| "blank".to_string()),
                )
                .to_string();
            }
            Ok(Event::Eof) | Err(_) => break,
            Ok(_) => {}
        }
        buffer.clear();
    }
    "blank".to_string()
}

fn placeholder_text(element: &ElementBuilder) -> String {
    if !element.text.trim().is_empty() {
        return element.text.trim_end_matches('\n').to_string();
    }
    match element.placeholder_type.as_deref() {
        Some("title") | Some("ctrTitle") => "Click to add Title".to_string(),
        Some("body") | Some("obj") | Some("subTitle") => "Click to add Text".to_string(),
        _ => String::new(),
    }
}

fn image_placeholder_element(
    element: &ElementBuilder,
    slide_number: usize,
    z_index: i32,
    timings: &HashMap<u32, AnimationTiming>,
) -> SlideElement {
    let timing = animation_timing(timings, element.shape_id);
    SlideElement {
        id: format!("pptx-{slide_number}-{z_index}"),
        x: element.x,
        y: element.y,
        width: element.width.max(120.0),
        height: element.height.max(80.0),
        rotation: element.rotation,
        z_index,
        entrance: timing
            .map(|value| value.kind.as_str().to_string())
            .unwrap_or_else(|| "none".to_string()),
        entrance_delay_ms: timing.and_then(|value| value.delay_ms),
        entrance_duration_ms: timing.and_then(|value| value.duration_ms),
        entrance_order: timing.and_then(|value| value.order),
        exit: "none".to_string(),
        exit_duration_ms: None,
        hyperlink: element.hyperlink.clone(),
        kind: ElementKind::Shape {
            shape_type: "rect".to_string(),
            fill_color: "#f8fafc".to_string(),
            stroke_color: "#64748b".to_string(),
            stroke_width: 1.0,
            text: "Click to add Picture".to_string(),
            fill_gradient: None,
            shadow: false,
            font_family: None,
            bold: false,
            italic: false,
            underline: false,
        },
    }
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

fn safe_external_hyperlink_target(target: &str) -> Option<String> {
    let target = target.trim();
    if target.is_empty()
        || target.chars().count() > MAX_HYPERLINK_CHARS
        || target.chars().any(|ch| ch.is_control())
    {
        return None;
    }
    let lower = target.to_ascii_lowercase();
    if lower.starts_with("http://") || lower.starts_with("https://") {
        let authority = target
            .split_once("://")
            .and_then(|(_, rest)| rest.split(['/', '?', '#']).next())
            .unwrap_or_default()
            .trim();
        if !authority.is_empty() && !authority.chars().any(char::is_whitespace) {
            return Some(target.to_string());
        }
        return None;
    }
    if lower.starts_with("mailto:") {
        let address = target["mailto:".len()..].split('?').next()?.trim();
        if address.contains('@') && !address.starts_with('@') && !address.ends_with('@') {
            return Some(target.to_string());
        }
        return None;
    }
    if lower.starts_with("tel:") {
        let number = target["tel:".len()..]
            .chars()
            .filter(|ch| !ch.is_ascii_whitespace() && *ch != '-' && *ch != '(' && *ch != ')')
            .collect::<String>();
        if !number.is_empty()
            && number
                .chars()
                .all(|ch| ch.is_ascii_digit() || ch == '+' || ch == '.')
        {
            return Some(target.to_string());
        }
    }
    None
}

fn read_external_relationships(
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
                let target_mode = attribute(&event, b"TargetMode");
                if let (Some(id), Some(target)) = (id, target) {
                    let is_external = target_mode
                        .as_deref()
                        .is_some_and(|mode| mode.eq_ignore_ascii_case("External"))
                        || target.starts_with("http://")
                        || target.starts_with("https://")
                        || target.starts_with("mailto:")
                        || target.starts_with("tel:");
                    if is_external {
                        if let Some(target) = safe_external_hyperlink_target(&target) {
                            relationships.insert(id, target);
                        }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AnimationKind {
    Fade,
    Zoom,
}

impl AnimationKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Fade => "fade",
            Self::Zoom => "zoom",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct AnimationTiming {
    kind: AnimationKind,
    delay_ms: Option<u32>,
    duration_ms: Option<u32>,
    order: Option<u32>,
    /// `true` for `transition="out"` effects (exit animations).
    is_exit: bool,
}

fn parse_animation_number(value: Option<String>) -> Option<u32> {
    value
        .and_then(|value| value.parse::<u64>().ok())
        .and_then(|value| u32::try_from(value).ok())
}

fn parse_animation_delay(value: Option<String>) -> Option<u32> {
    parse_animation_number(value).map(|value| value.min(60_000))
}

fn parse_animation_duration(value: Option<String>) -> Option<u32> {
    parse_animation_number(value).map(|value| value.clamp(50, 60_000))
}

/// Return timing metadata for shape ids targeted by the supported native fade
/// or zoom entrance effects and fade exits. Unknown or malformed timeline
/// nodes are ignored so drawing import remains non-fatal.
fn animation_timings(xml: &[u8]) -> HashMap<u32, AnimationTiming> {
    let mut reader = Reader::from_reader(xml);
    let mut buffer = Vec::new();
    let mut fade_effect_depth = 0usize;
    let mut current_kind = None;
    let mut current_exit = false;
    let mut current_target = None;
    let mut current_delay_ms = None;
    let mut current_duration_ms = None;
    let mut pending_delay_ms = None;
    let mut next_order = 0u32;
    let mut timings: HashMap<u32, AnimationTiming> = HashMap::new();
    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(event)) | Ok(Event::Empty(event)) => {
                let name = local_name(event.name().as_ref()).to_vec();
                let transition = attribute(&event, b"transition");
                let is_in = transition.as_deref() == Some("in");
                let is_out = transition.as_deref() == Some("out");
                let supports_effect = is_in || is_out;
                match name.as_slice() {
                    b"animEffect" if supports_effect => {
                        let kind = match attribute(&event, b"filter").as_deref() {
                            Some("fade") => Some(AnimationKind::Fade),
                            Some("zoom") | Some("zoom(in)") if is_in => Some(AnimationKind::Zoom),
                            _ => None,
                        };
                        let Some(kind) = kind else {
                            buffer.clear();
                            continue;
                        };
                        fade_effect_depth = fade_effect_depth.saturating_add(1);
                        if fade_effect_depth == 1 {
                            current_kind = Some(kind);
                            current_exit = is_out;
                            current_target = None;
                            current_delay_ms = pending_delay_ms.take();
                            current_duration_ms = None;
                        }
                    }
                    b"cond" if fade_effect_depth > 0 => {
                        current_delay_ms = parse_animation_delay(attribute(&event, b"delay"));
                    }
                    b"cond" => {
                        pending_delay_ms = parse_animation_delay(attribute(&event, b"delay"));
                    }
                    b"cTn" if fade_effect_depth > 0 => {
                        current_duration_ms = parse_animation_duration(attribute(&event, b"dur"));
                    }
                    b"spTgt" if fade_effect_depth > 0 => {
                        current_target =
                            attribute(&event, b"spid").and_then(|value| value.parse::<u32>().ok());
                    }
                    _ => {}
                }
            }
            Ok(Event::End(event)) => {
                if local_name(event.name().as_ref()) == b"animEffect" && fade_effect_depth > 0 {
                    if fade_effect_depth == 1 {
                        if let Some(shape_id) = current_target {
                            let timing = AnimationTiming {
                                kind: current_kind.unwrap_or(AnimationKind::Fade),
                                delay_ms: current_delay_ms,
                                duration_ms: current_duration_ms,
                                order: Some(next_order),
                                is_exit: current_exit,
                            };
                            // Exits only apply when no entrance targeted the same shape.
                            let slot_free = !timings.contains_key(&shape_id);
                            if slot_free || (current_exit && !timings[&shape_id].is_exit) {
                                timings.insert(shape_id, timing);
                            }
                            next_order = next_order.saturating_add(1);
                        }
                        current_target = None;
                        current_kind = None;
                        current_exit = false;
                        current_delay_ms = None;
                        current_duration_ms = None;
                    }
                    fade_effect_depth -= 1;
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            Ok(_) => {}
        }
        buffer.clear();
    }
    timings
}

fn animation_timing(
    timings: &HashMap<u32, AnimationTiming>,
    shape_id: Option<u32>,
) -> Option<AnimationTiming> {
    shape_id.and_then(|id| timings.get(&id).copied())
}

fn animation_entrance(timings: &HashMap<u32, AnimationTiming>, shape_id: Option<u32>) -> String {
    match animation_timing(timings, shape_id) {
        Some(timing) if !timing.is_exit => timing.kind.as_str().to_string(),
        _ => "none".to_string(),
    }
}

fn animation_exit(timings: &HashMap<u32, AnimationTiming>, shape_id: Option<u32>) -> String {
    match animation_timing(timings, shape_id) {
        Some(timing) if timing.is_exit => "fade".to_string(),
        _ => "none".to_string(),
    }
}

fn animation_exit_duration(
    timings: &HashMap<u32, AnimationTiming>,
    shape_id: Option<u32>,
) -> Option<u32> {
    animation_timing(timings, shape_id)
        .filter(|timing| timing.is_exit)
        .and_then(|timing| timing.duration_ms)
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

fn parse_slide_transition(xml: &[u8]) -> String {
    let mut reader = Reader::from_reader(xml);
    let mut buffer = Vec::new();
    let mut in_transition = false;
    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(event)) | Ok(Event::Empty(event)) => {
                let name = local_name(event.name().as_ref()).to_vec();
                if name.as_slice() == b"transition" {
                    in_transition = true;
                } else if in_transition {
                    let transition = match name.as_slice() {
                        b"fade" => Some("fade"),
                        b"dissolve" => Some("dissolve"),
                        // PowerPoint 2016+ morph transition (p14 extension).
                        b"prstTrans" if attribute(&event, b"prst").as_deref() == Some("morph") => {
                            Some("morph")
                        }
                        b"zoom" if attribute(&event, b"dir").as_deref() != Some("out") => {
                            Some("zoom")
                        }
                        b"push" => match attribute(&event, b"dir").as_deref() {
                            Some("l") => Some("slide-left"),
                            Some("r") => Some("slide-right"),
                            _ => None,
                        },
                        b"wipe" => match attribute(&event, b"dir").as_deref() {
                            Some("l") => Some("wipe-left"),
                            Some("r") => Some("wipe-right"),
                            _ => None,
                        },
                        _ => None,
                    };
                    if let Some(transition) = transition {
                        return transition.to_string();
                    }
                }
            }
            Ok(Event::End(event)) if local_name(event.name().as_ref()) == b"transition" => {
                in_transition = false;
            }
            Ok(Event::Eof) | Err(_) => break,
            Ok(_) => {}
        }
        buffer.clear();
    }
    "none".to_string()
}

fn parse_slide(
    xml: &[u8],
    slide_number: usize,
    archive: &mut zip::ZipArchive<std::fs::File>,
    relationships: &HashMap<String, String>,
    external_relationships: &HashMap<String, String>,
    warnings: &mut Vec<String>,
    media_bytes: &mut u64,
) -> Result<redoc_slide_engine::Slide, ExportError> {
    let layout = parse_slide_layout_name(archive, relationships);
    let mut reader = Reader::from_reader(xml);
    reader.config_mut().trim_text(true);
    let mut buffer = Vec::new();
    let mut stack: Vec<Vec<u8>> = Vec::new();
    let mut current: Option<ElementBuilder> = None;
    let mut in_text = false;
    let mut color_context = None::<&'static str>;
    let mut elements = Vec::new();
    let mut z_index = 0i32;
    let animation_timings = animation_timings(xml);

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
                    b"ph" => {
                        if let Some(element) = current.as_mut() {
                            element.placeholder_type = attribute(&event, b"type");
                        }
                    }
                    b"hlinkClick" => {
                        if let Some(element) = current.as_mut() {
                            if let Some(rel_id) = attribute(&event, b"id") {
                                element.hyperlink = external_relationships.get(&rel_id).cloned();
                            }
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
                            let preset =
                                attribute(&event, b"prst").unwrap_or_else(|| "rect".to_string());
                            element.shape_type = canonical_shape_type(&preset)
                                .map(str::to_string)
                                .unwrap_or_else(|| {
                                    warnings.push(format!(
                                        "slide {slide_number}: shape preset '{preset}' is unsupported; imported as a rectangle"
                                    ));
                                    "rect".to_string()
                                });
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
                        element.shape_id =
                            attribute(&event, b"id").and_then(|value| value.parse::<u32>().ok());
                    }
                } else if name.as_slice() == b"ph" {
                    if let Some(element) = current.as_mut() {
                        element.placeholder_type = attribute(&event, b"type");
                    }
                } else if name.as_slice() == b"hlinkClick" {
                    if let Some(element) = current.as_mut() {
                        if let Some(rel_id) = attribute(&event, b"id") {
                            element.hyperlink = external_relationships.get(&rel_id).cloned();
                        }
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
                            let is_picture_placeholder = element
                                .placeholder_type
                                .as_deref()
                                .is_some_and(|value| value.eq_ignore_ascii_case("pic"));
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
                                        if is_picture_placeholder {
                                            elements.push(image_placeholder_element(
                                                &element,
                                                slide_number,
                                                z_index,
                                                &animation_timings,
                                            ));
                                            z_index += 1;
                                        }
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
                                                    entrance: animation_entrance(
                                                        &animation_timings,
                                                        element.shape_id,
                                                    ),
                                                    entrance_delay_ms: animation_timing(
                                                        &animation_timings,
                                                        element.shape_id,
                                                    )
                                                    .and_then(|timing| timing.delay_ms),
                                                    entrance_duration_ms: animation_timing(
                                                        &animation_timings,
                                                        element.shape_id,
                                                    )
                                                    .and_then(|timing| timing.duration_ms),
                                                    entrance_order: animation_timing(
                                                        &animation_timings,
                                                        element.shape_id,
                                                    )
                                                    .and_then(|timing| timing.order),
                                                    exit: animation_exit(
                                                        &animation_timings,
                                                        element.shape_id,
                                                    ),
                                                    exit_duration_ms: animation_exit_duration(
                                                        &animation_timings,
                                                        element.shape_id,
                                                    ),
                                                    hyperlink: element.hyperlink.clone(),
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
                                            Err(error) => {
                                                warnings.push(format!(
                                                    "slide {slide_number}: image {media_path} skipped ({error})"
                                                ));
                                                if is_picture_placeholder {
                                                    elements.push(image_placeholder_element(
                                                        &element,
                                                        slide_number,
                                                        z_index,
                                                        &animation_timings,
                                                    ));
                                                    z_index += 1;
                                                }
                                            }
                                        }
                                    }
                                } else {
                                    warnings.push(format!(
                                        "slide {slide_number}: image relationship {rel_id} was missing"
                                    ));
                                    if is_picture_placeholder {
                                        elements.push(image_placeholder_element(
                                            &element,
                                            slide_number,
                                            z_index,
                                            &animation_timings,
                                        ));
                                        z_index += 1;
                                    }
                                }
                            } else if is_picture_placeholder {
                                elements.push(image_placeholder_element(
                                    &element,
                                    slide_number,
                                    z_index,
                                    &animation_timings,
                                ));
                                z_index += 1;
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
                                entrance: animation_entrance(&animation_timings, element.shape_id),
                                entrance_delay_ms: animation_timing(
                                    &animation_timings,
                                    element.shape_id,
                                )
                                .and_then(|timing| timing.delay_ms),
                                entrance_duration_ms: animation_timing(
                                    &animation_timings,
                                    element.shape_id,
                                )
                                .and_then(|timing| timing.duration_ms),
                                entrance_order: animation_timing(
                                    &animation_timings,
                                    element.shape_id,
                                )
                                .and_then(|timing| timing.order),
                                exit: animation_exit(&animation_timings, element.shape_id),
                                exit_duration_ms: animation_exit_duration(
                                    &animation_timings,
                                    element.shape_id,
                                ),
                                hyperlink: element.hyperlink.clone(),
                                kind: ElementKind::Shape {
                                    shape_type: shape_type.to_string(),
                                    fill_color: element.fill_color,
                                    stroke_color: element.stroke_color,
                                    stroke_width: element.stroke_width.max(0.1),
                                    text: element.text.trim_end_matches('\n').to_string(),
                                    fill_gradient: None,
                                    shadow: false,
                                    font_family: None,
                                    bold: false,
                                    italic: false,
                                    underline: false,
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
                                entrance: animation_entrance(&animation_timings, element.shape_id),
                                entrance_delay_ms: animation_timing(
                                    &animation_timings,
                                    element.shape_id,
                                )
                                .and_then(|timing| timing.delay_ms),
                                entrance_duration_ms: animation_timing(
                                    &animation_timings,
                                    element.shape_id,
                                )
                                .and_then(|timing| timing.duration_ms),
                                entrance_order: animation_timing(
                                    &animation_timings,
                                    element.shape_id,
                                )
                                .and_then(|timing| timing.order),
                                exit: animation_exit(&animation_timings, element.shape_id),
                                exit_duration_ms: animation_exit_duration(
                                    &animation_timings,
                                    element.shape_id,
                                ),
                                hyperlink: element.hyperlink.clone(),
                                kind: ElementKind::Table {
                                    rows,
                                    cols,
                                    data,
                                    merges: Vec::new(),
                                    header_row: false,
                                    table_style: None,
                                },
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
                                    entrance: animation_entrance(
                                        &animation_timings,
                                        element.shape_id,
                                    ),
                                    entrance_delay_ms: animation_timing(
                                        &animation_timings,
                                        element.shape_id,
                                    )
                                    .and_then(|timing| timing.delay_ms),
                                    entrance_duration_ms: animation_timing(
                                        &animation_timings,
                                        element.shape_id,
                                    )
                                    .and_then(|timing| timing.duration_ms),
                                    entrance_order: animation_timing(
                                        &animation_timings,
                                        element.shape_id,
                                    )
                                    .and_then(|timing| timing.order),
                                    exit: animation_exit(&animation_timings, element.shape_id),
                                    exit_duration_ms: animation_exit_duration(
                                        &animation_timings,
                                        element.shape_id,
                                    ),
                                    hyperlink: element.hyperlink.clone(),
                                    kind: ElementKind::Chart {
                                        chart_type,
                                        data,
                                        labels,
                                        legend: true,
                                        show_labels: true,
                                        show_axes: true,
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
                                    entrance: animation_entrance(
                                        &animation_timings,
                                        element.shape_id,
                                    ),
                                    entrance_delay_ms: animation_timing(
                                        &animation_timings,
                                        element.shape_id,
                                    )
                                    .and_then(|timing| timing.delay_ms),
                                    entrance_duration_ms: animation_timing(
                                        &animation_timings,
                                        element.shape_id,
                                    )
                                    .and_then(|timing| timing.duration_ms),
                                    entrance_order: animation_timing(
                                        &animation_timings,
                                        element.shape_id,
                                    )
                                    .and_then(|timing| timing.order),
                                    exit: animation_exit(&animation_timings, element.shape_id),
                                    exit_duration_ms: animation_exit_duration(
                                        &animation_timings,
                                        element.shape_id,
                                    ),
                                    hyperlink: element.hyperlink.clone(),
                                    kind: ElementKind::Shape {
                                        shape_type: "rect".to_string(),
                                        fill_color: "#fef3c7".to_string(),
                                        stroke_color: "#d97706".to_string(),
                                        stroke_width: 1.0,
                                        text: "[Unsupported chart]".to_string(),
                                        fill_gradient: None,
                                        shadow: false,
                                        font_family: None,
                                        bold: false,
                                        italic: false,
                                        underline: false,
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
                                entrance: animation_entrance(&animation_timings, element.shape_id),
                                entrance_delay_ms: animation_timing(
                                    &animation_timings,
                                    element.shape_id,
                                )
                                .and_then(|timing| timing.delay_ms),
                                entrance_duration_ms: animation_timing(
                                    &animation_timings,
                                    element.shape_id,
                                )
                                .and_then(|timing| timing.duration_ms),
                                entrance_order: animation_timing(
                                    &animation_timings,
                                    element.shape_id,
                                )
                                .and_then(|timing| timing.order),
                                exit: animation_exit(&animation_timings, element.shape_id),
                                exit_duration_ms: animation_exit_duration(
                                    &animation_timings,
                                    element.shape_id,
                                ),
                                hyperlink: element.hyperlink.clone(),
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
                                    fill_gradient: None,
                                    shadow: false,
                                    font_family: None,
                                    bold: false,
                                    italic: false,
                                    underline: false,
                                },
                            });
                            z_index += 1;
                        } else {
                            let text = placeholder_text(&element);
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
                                    entrance: animation_entrance(
                                        &animation_timings,
                                        element.shape_id,
                                    ),
                                    entrance_delay_ms: animation_timing(
                                        &animation_timings,
                                        element.shape_id,
                                    )
                                    .and_then(|timing| timing.delay_ms),
                                    entrance_duration_ms: animation_timing(
                                        &animation_timings,
                                        element.shape_id,
                                    )
                                    .and_then(|timing| timing.duration_ms),
                                    entrance_order: animation_timing(
                                        &animation_timings,
                                        element.shape_id,
                                    )
                                    .and_then(|timing| timing.order),
                                    exit: animation_exit(&animation_timings, element.shape_id),
                                    exit_duration_ms: animation_exit_duration(
                                        &animation_timings,
                                        element.shape_id,
                                    ),
                                    hyperlink: element.hyperlink.clone(),
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
                                    entrance: animation_entrance(
                                        &animation_timings,
                                        element.shape_id,
                                    ),
                                    entrance_delay_ms: animation_timing(
                                        &animation_timings,
                                        element.shape_id,
                                    )
                                    .and_then(|timing| timing.delay_ms),
                                    entrance_duration_ms: animation_timing(
                                        &animation_timings,
                                        element.shape_id,
                                    )
                                    .and_then(|timing| timing.duration_ms),
                                    entrance_order: animation_timing(
                                        &animation_timings,
                                        element.shape_id,
                                    )
                                    .and_then(|timing| timing.order),
                                    exit: animation_exit(&animation_timings, element.shape_id),
                                    exit_duration_ms: animation_exit_duration(
                                        &animation_timings,
                                        element.shape_id,
                                    ),
                                    hyperlink: element.hyperlink.clone(),
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
                                        fill_gradient: None,
                                        shadow: false,
                                        font_family: None,
                                        bold: false,
                                        italic: false,
                                        underline: false,
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

    let transition = parse_slide_transition(xml);

    Ok(redoc_slide_engine::Slide {
        id: format!("pptx-slide-{slide_number}"),
        layout,
        elements,
        notes: String::new(),
        bg_override: parse_slide_background(xml),
        transition,
        comments: Vec::new(),
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

/// Parse a `ppt/comments/commentNcomment.xml` part into slide comments.
/// Author names come from the presentation's `p:cmAuthorLst` mapping; when
/// that is unavailable the raw authorId is shown as a fallback.
fn parse_slide_comments(
    xml: &[u8],
    author_names: &[String],
) -> Result<Vec<redoc_slide_engine::SlideCommentModel>, ExportError> {
    const MAX_COMMENTS: usize = 200;
    let mut reader = Reader::from_reader(xml);
    reader.config_mut().trim_text(true);
    let mut buffer = Vec::new();
    let mut comments = Vec::new();
    let mut in_text = false;
    let mut text = String::new();
    let mut current: Option<(usize, String, bool)> = None; // (authorId, created, resolved)
    let mut counter = 0usize;
    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(event)) | Ok(Event::Empty(event)) => {
                match local_name(event.name().as_ref()) {
                    b"cm" => {
                        if comments.len() >= MAX_COMMENTS {
                            break;
                        }
                        let author_id = attribute(&event, b"authorId")
                            .and_then(|value| value.parse::<usize>().ok())
                            .unwrap_or(0);
                        let created = attribute(&event, b"dt")
                            .unwrap_or_else(|| "1970-01-01T00:00:00Z".into());
                        let resolved = attribute(&event, b"resolved").as_deref() == Some("1");
                        current = Some((author_id, created, resolved));
                    }
                    b"text" if current.is_some() => in_text = true,
                    _ => {}
                }
            }
            Ok(Event::Text(event)) if in_text => {
                text.push_str(
                    &event
                        .unescape()
                        .map_err(|error| invalid_data(format!("invalid comment XML: {error}")))?,
                );
            }
            Ok(Event::End(event)) => match local_name(event.name().as_ref()) {
                b"text" => in_text = false,
                b"cm" => {
                    if let Some((author_id, created, resolved)) = current.take() {
                        let body = text.trim().to_string();
                        if !body.is_empty() {
                            counter += 1;
                            comments.push(redoc_slide_engine::SlideCommentModel {
                                id: format!("pptx-comment-{counter}"),
                                author: author_names
                                    .get(author_id)
                                    .cloned()
                                    .unwrap_or_else(|| format!("Author {author_id}")),
                                text: body,
                                resolved,
                                created_at: created,
                            });
                        }
                    }
                    text.clear();
                }
                _ => {}
            },
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => return Err(invalid_data(format!("invalid comment XML: {error}"))),
        }
        buffer.clear();
    }
    Ok(comments)
}

/// Parse `p:cmAuthorLst` from presentation.xml into an author-id → name list.
fn parse_comment_authors(xml: &[u8]) -> Vec<String> {
    let mut reader = Reader::from_reader(xml);
    reader.config_mut().trim_text(true);
    let mut buffer = Vec::new();
    let mut authors = Vec::<(usize, String)>::new();
    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(event)) | Ok(Event::Empty(event))
                if local_name(event.name().as_ref()) == b"cmAuthor" =>
            {
                let id = attribute(&event, b"id")
                    .and_then(|value| value.parse::<usize>().ok())
                    .unwrap_or(authors.len());
                let name = attribute(&event, b"name").unwrap_or_else(|| "You".into());
                authors.push((id, name));
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(_) => break,
        }
        buffer.clear();
    }
    let mut names = vec![String::from("You"); authors.len().max(1)];
    for (id, name) in authors {
        if id < names.len() {
            names[id] = name;
        }
    }
    names
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
                        if let (Some(slot), Some(value)) = (color_slot, attribute(&event, b"val")) {
                            let value = value.trim();
                            if value.len() == 6
                                && value.bytes().all(|byte| byte.is_ascii_hexdigit())
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
                    if let (Some(slot), Some(value)) = (color_slot, attribute(&event, b"val")) {
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
    let author_names = parse_comment_authors(&presentation);
    let mut slides = Vec::with_capacity(slide_numbers.len());
    let mut media_bytes = 0u64;
    for slide_number in slide_numbers {
        let slide_path = format!("ppt/slides/slide{slide_number}.xml");
        let xml = read_entry(&mut archive, &slide_path, MAX_XML_BYTES)?;
        let relationships = read_relationships(&mut archive, slide_number)?;
        let external_relationships = read_external_relationships(&mut archive, slide_number)?;
        let mut slide = parse_slide(
            &xml,
            slide_number,
            &mut archive,
            &relationships,
            &external_relationships,
            &mut warnings,
            &mut media_bytes,
        )?;
        let notes_path = format!("ppt/notesSlides/notesSlide{slide_number}.xml");
        if let Ok(notes_xml) = read_entry(&mut archive, &notes_path, MAX_XML_BYTES) {
            slide.notes = parse_notes(&notes_xml)?;
        }
        let comments_path = format!("ppt/comments/comment{slide_number}comment.xml");
        if let Ok(comments_xml) = read_entry(&mut archive, &comments_path, MAX_XML_BYTES) {
            slide.comments = parse_slide_comments(&comments_xml, &author_names)?;
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
    fn parses_and_bounds_native_animation_timing() {
        let xml = br#"<p:sld xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"><p:timing><p:par><p:cTn><p:stCondLst><p:cond delay="999999"/></p:stCondLst><p:childTnLst><p:animEffect transition="in" filter="fade"><p:cBhvr><p:cTn dur="1"/><p:tgtEl><p:spTgt spid="7"/></p:tgtEl></p:cBhvr></p:animEffect></p:childTnLst></p:cTn></p:par></p:timing></p:sld>"#;
        let timings = animation_timings(xml);
        assert_eq!(
            timings.get(&7),
            Some(&AnimationTiming {
                kind: AnimationKind::Fade,
                delay_ms: Some(60_000),
                duration_ms: Some(50),
                order: Some(0),
                is_exit: false,
            })
        );
    }

    #[test]
    fn parses_native_exit_animation_as_fade_exit() {
        let xml = br#"<p:sld xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"><p:timing><p:par><p:cTn><p:childTnLst><p:animEffect transition="out" filter="fade"><p:cBhvr><p:cTn dur="500"/><p:tgtEl><p:spTgt spid="9"/></p:tgtEl></p:cBhvr></p:animEffect></p:childTnLst></p:cTn></p:par></p:timing></p:sld>"#;
        let timings = animation_timings(xml);
        let timing = timings.get(&9).expect("exit timing parsed");
        assert!(timing.is_exit);
        assert_eq!(timing.kind, AnimationKind::Fade);
        assert_eq!(timing.duration_ms, Some(500));
        assert_eq!(animation_exit(&timings, Some(9)), "fade");
        assert_eq!(animation_entrance(&timings, Some(9)), "none");
    }

    #[test]
    fn parses_native_zoom_animation_kind() {
        let xml = br#"<p:sld xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"><p:timing><p:par><p:cTn><p:childTnLst><p:animEffect transition="in" filter="zoom(in)"><p:cBhvr><p:cTn dur="350"/><p:tgtEl><p:spTgt spid="7"/></p:tgtEl></p:cBhvr></p:animEffect></p:childTnLst></p:cTn></p:par></p:timing></p:sld>"#;
        let timings = animation_timings(xml);
        assert_eq!(
            timings.get(&7).map(|timing| timing.kind),
            Some(AnimationKind::Zoom)
        );
    }

    #[test]
    fn canonicalizes_supported_shape_presets_and_rejects_unknown_values() {
        assert_eq!(canonical_shape_type("rect"), Some("rect"));
        assert_eq!(canonical_shape_type("roundRect"), Some("roundedRect"));
        assert_eq!(canonical_shape_type("STAR5"), Some("star"));
        assert_eq!(canonical_shape_type("hexagon"), None);
    }

    #[test]
    fn filters_unsafe_external_hyperlink_targets() {
        assert_eq!(
            safe_external_hyperlink_target("https://example.com/path"),
            Some("https://example.com/path".to_string())
        );
        assert!(safe_external_hyperlink_target("javascript:alert(1)").is_none());
        assert!(safe_external_hyperlink_target("https://").is_none());
        assert!(safe_external_hyperlink_target(&format!(
            "https://example.com/{}",
            "x".repeat(2_050)
        ))
        .is_none());
    }

    #[test]
    fn parses_supported_native_transitions() {
        let cases = [
            (&br#"<p:sld xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"><p:transition><p:fade/></p:transition></p:sld>"#[..], "fade"),
            (&br#"<p:sld xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"><p:transition><p:push dir="l"/></p:transition></p:sld>"#[..], "slide-left"),
            (&br#"<p:sld xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"><p:transition><p:push dir="r"/></p:transition></p:sld>"#[..], "slide-right"),
            (&br#"<p:sld xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"><p:transition><p:wipe dir="l"/></p:transition></p:sld>"#[..], "wipe-left"),
            (&br#"<p:sld xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"><p:transition><p:wipe dir="r"/></p:transition></p:sld>"#[..], "wipe-right"),
            (&br#"<p:sld xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"><p:transition><p:zoom dir="in"/></p:transition></p:sld>"#[..], "zoom"),
            (&br#"<p:sld xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"><p:transition><p:dissolve/></p:transition></p:sld>"#[..], "dissolve"),
            (&br#"<p:sld xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:p14="http://schemas.microsoft.com/office/powerpoint/2010/main"><p:transition><p14:prstTrans prst="morph" option="byObject"/></p:transition></p:sld>"#[..], "morph"),
        ];
        for (xml, expected) in cases {
            assert_eq!(parse_slide_transition(xml), expected);
        }
        // A non-morph prstTrans is not mapped.
        assert_eq!(
            parse_slide_transition(br#"<p:sld xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:p14="http://schemas.microsoft.com/office/powerpoint/2010/main"><p:transition><p14:prstTrans prst="ripple"/></p:transition></p:sld>"#),
            "none"
        );
        assert_eq!(
            parse_slide_transition(br#"<p:sld xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"><p:transition><p:zoom dir="out"/></p:transition></p:sld>"#),
            "none"
        );
    }

    #[test]
    fn round_trips_extended_transition_through_pptx() {
        let mut source = DeckModel::new_default();
        source.slides[0].transition = "wipe-right".to_string();
        let bytes = crate::pptx::export_deck_to_pptx(&source).expect("export transition deck");
        let path = std::env::temp_dir().join(format!(
            "redoc-pptx-transition-roundtrip-{}.pptx",
            std::process::id()
        ));
        std::fs::write(&path, bytes).expect("write transition deck");
        let result = import_deck_from_pptx_with_report(&path).expect("import transition deck");
        let _ = std::fs::remove_file(path);

        assert_eq!(result.deck.slides[0].transition, "wipe-right");
        assert!(result
            .warnings
            .iter()
            .all(|warning| !warning.contains("transition")));
    }

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
            entrance_delay_ms: None,
            entrance_duration_ms: None,
            entrance_order: None,
            exit: "none".to_string(),
            exit_duration_ms: None,
            hyperlink: Some("https://example.com/slide".to_string()),
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
        source.slides[0].elements[0].entrance_delay_ms = Some(120);
        source.slides[0].elements[0].entrance_duration_ms = Some(900);
        source.slides[0].elements[0].entrance_order = Some(2);
        source.slides[0].elements.push(SlideElement {
            id: "shape".to_string(),
            x: 40.0,
            y: 180.0,
            width: 120.0,
            height: 80.0,
            rotation: 15.0,
            z_index: 2,
            entrance: "none".to_string(),
            entrance_delay_ms: None,
            entrance_duration_ms: None,
            entrance_order: None,
            exit: "none".to_string(),
            exit_duration_ms: None,
            hyperlink: None,
            kind: ElementKind::Shape {
                shape_type: "ellipse".to_string(),
                fill_color: "#00ff00".to_string(),
                stroke_color: "#0000ff".to_string(),
                stroke_width: 2.0,
                text: String::new(),
                fill_gradient: None,
                shadow: false,
                font_family: None,
                bold: false,
                italic: false,
                underline: false,
            },
        });
        source.slides[0].elements[1].entrance = "fade".to_string();
        source.slides[0].elements[1].entrance_delay_ms = Some(10);
        source.slides[0].elements[1].entrance_duration_ms = Some(500);
        source.slides[0].elements[1].entrance_order = Some(1);
        source.slides[0].elements.push(SlideElement {
            id: "image".to_string(),
            x: 220.0,
            y: 180.0,
            width: 100.0,
            height: 100.0,
            rotation: 0.0,
            z_index: 3,
            entrance: "none".to_string(),
            entrance_delay_ms: None,
            entrance_duration_ms: None,
            entrance_order: None,
            exit: "none".to_string(),
            exit_duration_ms: None,
            hyperlink: None,
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
        assert_eq!(
            result.deck.slides[0].elements[0].hyperlink.as_deref(),
            Some("https://example.com/slide")
        );
        assert!(result.deck.slides[0]
            .elements
            .iter()
            .any(|element| element.entrance == "fade"));
        let imported_animation = result.deck.slides[0]
            .elements
            .iter()
            .find(|element| element.entrance == "fade")
            .expect("fade element");
        assert_eq!(imported_animation.entrance_delay_ms, Some(120));
        assert_eq!(imported_animation.entrance_duration_ms, Some(900));
        assert_eq!(imported_animation.entrance_order, Some(1));
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
    fn round_trips_supported_slide_layouts() {
        let mut source = DeckModel::new_default();
        source.slides[0].layout = "title".to_string();
        source.add_slide("two_col");
        source.add_slide("section");

        let bytes = crate::pptx::export_deck_to_pptx(&source).expect("export layout deck");
        let mut archive =
            zip::ZipArchive::new(std::io::Cursor::new(bytes.clone())).expect("read layout archive");
        let mut title_layout = String::new();
        archive
            .by_name("ppt/slideLayouts/slideLayout1.xml")
            .expect("title layout part")
            .read_to_string(&mut title_layout)
            .expect("read title layout");
        assert!(title_layout.contains(r#"type="title""#));
        assert!(title_layout.contains(r#"<p:ph type="title""#));
        assert!(title_layout.contains(r#"<p:ph type="body" idx="1""#));
        let mut title_slide = String::new();
        archive
            .by_name("ppt/slides/slide1.xml")
            .expect("title slide part")
            .read_to_string(&mut title_slide)
            .expect("read title slide");
        assert!(title_slide.contains(r#"<p:ph type="title""#));
        assert!(title_slide.contains(r#"<p:ph type="body" idx="1""#));
        let mut two_col_layout = String::new();
        archive
            .by_name("ppt/slideLayouts/slideLayout2.xml")
            .expect("two-column layout part")
            .read_to_string(&mut two_col_layout)
            .expect("read two-column layout");
        assert_eq!(two_col_layout.matches(r#"<p:ph type="body""#).count(), 2);
        let path = std::env::temp_dir().join(format!(
            "redoc-pptx-layout-roundtrip-{}.pptx",
            std::process::id()
        ));
        std::fs::write(&path, bytes).expect("write layout deck");
        let result = import_deck_from_pptx_with_report(&path).expect("import layout deck");
        let _ = std::fs::remove_file(&path);

        assert_eq!(
            result
                .deck
                .slides
                .iter()
                .map(|slide| slide.layout.as_str())
                .collect::<Vec<_>>(),
            vec!["title", "two_col", "section"]
        );
        assert!(
            result.warnings.is_empty(),
            "unexpected warnings: {:?}",
            result.warnings
        );
    }

    #[test]
    fn round_trips_common_shape_presets_to_canonical_names() {
        let mut source = DeckModel::new_default();
        source.slides[0].elements.clear();
        for (index, shape_type) in ["roundedRect", "triangle", "diamond", "star"]
            .into_iter()
            .enumerate()
        {
            source.slides[0].elements.push(SlideElement {
                id: format!("shape-{shape_type}"),
                x: 40.0 + index as f64 * 120.0,
                y: 80.0,
                width: 100.0,
                height: 80.0,
                rotation: 0.0,
                z_index: index as i32 + 1,
                entrance: "none".to_string(),
                entrance_delay_ms: None,
                entrance_duration_ms: None,
                entrance_order: None,
                exit: "none".to_string(),
                exit_duration_ms: None,
                hyperlink: None,
                kind: ElementKind::Shape {
                    shape_type: shape_type.to_string(),
                    fill_color: "#336699".to_string(),
                    stroke_color: "#112233".to_string(),
                    stroke_width: 1.0,
                    text: if shape_type == "triangle" {
                        "Triangle label".to_string()
                    } else {
                        String::new()
                    },
                    fill_gradient: None,
                    shadow: false,
                    font_family: None,
                    bold: false,
                    italic: false,
                    underline: false,
                },
            });
        }

        let bytes = crate::pptx::export_deck_to_pptx(&source).expect("export shape deck");
        let path = std::env::temp_dir().join(format!(
            "redoc-pptx-shape-roundtrip-{}.pptx",
            std::process::id()
        ));
        std::fs::write(&path, bytes).expect("write shape deck");
        let result = import_deck_from_pptx_with_report(&path).expect("import shape deck");
        let _ = std::fs::remove_file(&path);

        let imported = result.deck.slides[0]
            .elements
            .iter()
            .filter_map(|element| match &element.kind {
                ElementKind::Shape {
                    shape_type, text, ..
                } => Some((shape_type.as_str(), text.as_str())),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            imported,
            vec![
                ("roundedRect", ""),
                ("triangle", "Triangle label"),
                ("diamond", ""),
                ("star", ""),
            ]
        );
        assert!(
            result.warnings.is_empty(),
            "unexpected warnings: {:?}",
            result.warnings
        );
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
            rotation: 15.0,
            z_index: 1,
            entrance: "none".to_string(),
            entrance_delay_ms: None,
            entrance_duration_ms: None,
            entrance_order: None,
            exit: "none".to_string(),
            exit_duration_ms: None,
            hyperlink: None,
            kind: ElementKind::Table {
                rows: 2,
                cols: 2,
                data: vec![
                    vec!["Name".to_string(), "Value".to_string()],
                    vec!["A & B".to_string(), "42".to_string()],
                ],
                merges: Vec::new(),
                header_row: false,
                table_style: None,
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
                ElementKind::Table {
                    rows, cols, data, ..
                } => Some((element.rotation, *rows, *cols, data.clone())),
                _ => None,
            })
            .expect("native table element");
        assert!((imported.0 - 15.0).abs() < 0.01);
        assert_eq!(imported.1, 2);
        assert_eq!(imported.2, 2);
        assert_eq!(imported.3[0], vec!["Name", "Value"]);
        assert_eq!(imported.3[1], vec!["A & B", "42"]);
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
            entrance_delay_ms: None,
            entrance_duration_ms: None,
            entrance_order: None,
            exit: "none".to_string(),
            exit_duration_ms: None,
            hyperlink: None,
            kind: ElementKind::Chart {
                chart_type: "pie".to_string(),
                data: vec![1.0, 2.0, 3.5],
                labels: vec![
                    "Mix".to_string(),
                    "A".to_string(),
                    "B".to_string(),
                    "C".to_string(),
                ],
                legend: true,
                show_labels: true,
                show_axes: true,
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
                    ..
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
        assert_eq!(
            result.deck.slides[0].bg_override.as_deref(),
            Some("#334455")
        );
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
        let zip = zip::ZipWriter::new(file);
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
    fn imports_empty_native_placeholders_as_editable_text() {
        let path = std::env::temp_dir().join(format!(
            "redoc-pptx-empty-placeholder-{}.pptx",
            std::process::id()
        ));
        let file = std::fs::File::create(&path).expect("create placeholder archive");
        let zip = zip::ZipWriter::new(file);
        zip.finish().expect("finish placeholder archive");
        let file = std::fs::File::open(&path).expect("open placeholder archive");
        let mut archive = zip::ZipArchive::new(file).expect("read placeholder archive");
        let mut warnings = Vec::new();
        let mut media_bytes = 0;
        let slide = parse_slide(
            br#"<p:sld xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><p:sp><p:nvSpPr><p:cNvPr id="2" name="Title Placeholder 1"/><p:cNvSpPr/><p:nvPr><p:ph type="title"/></p:nvPr></p:nvSpPr><p:spPr><a:xfrm><a:off x="95250" y="190500"/><a:ext cx="381000" cy="76200"/></a:xfrm></p:spPr><p:txBody><a:bodyPr/><a:lstStyle/><a:p/></p:txBody></p:sp></p:sld>"#,
            1,
            &mut archive,
            &HashMap::new(),
            &HashMap::new(),
            &mut warnings,
            &mut media_bytes,
        )
        .expect("parse placeholder slide");
        let _ = std::fs::remove_file(path);

        assert!(slide.elements.iter().any(|element| matches!(
            &element.kind,
            ElementKind::Text { text, .. } if text == "Click to add Title"
        )));
        assert!(warnings.is_empty(), "unexpected warnings: {:?}", warnings);
    }

    #[test]
    fn preserves_empty_native_picture_placeholders_as_editable_shapes() {
        let path = std::env::temp_dir().join(format!(
            "redoc-pptx-empty-picture-placeholder-{}.pptx",
            std::process::id()
        ));
        let file = std::fs::File::create(&path).expect("create picture placeholder archive");
        let zip = zip::ZipWriter::new(file);
        zip.finish().expect("finish picture placeholder archive");
        let file = std::fs::File::open(&path).expect("open picture placeholder archive");
        let mut archive = zip::ZipArchive::new(file).expect("read picture placeholder archive");
        let mut warnings = Vec::new();
        let mut media_bytes = 0;
        let slide = parse_slide(
            br#"<p:sld xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><p:pic><p:nvPicPr><p:cNvPr id="4" name="Picture Placeholder 1"/><p:cNvPicPr/><p:nvPr><p:ph type="pic"/></p:nvPr></p:nvPicPr><p:blipFill><a:blip/></p:blipFill><p:spPr><a:xfrm><a:off x="95250" y="190500"/><a:ext cx="381000" cy="190500"/></a:xfrm></p:spPr></p:pic></p:sld>"#,
            1,
            &mut archive,
            &HashMap::new(),
            &HashMap::new(),
            &mut warnings,
            &mut media_bytes,
        )
        .expect("parse picture placeholder slide");
        let _ = std::fs::remove_file(path);

        assert!(slide.elements.iter().any(|element| matches!(
            &element.kind,
            ElementKind::Shape { text, .. } if text == "Click to add Picture"
        )));
        assert!(warnings.is_empty(), "unexpected warnings: {:?}", warnings);
    }

    #[test]
    fn rejects_pptx_archives_with_too_many_entries() {
        let path = std::env::temp_dir().join(format!(
            "redoc-pptx-entry-limit-{}.pptx",
            std::process::id()
        ));
        let file = std::fs::File::create(&path).expect("create oversized archive");
        let mut zip = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default();
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
