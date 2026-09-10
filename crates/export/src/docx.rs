use crate::base64_util::{decode_base64, encode_base64};
use crate::pdf::ExportError;
use docx_rs::*;
use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, HashMap};
use std::io::{Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Debug)]
pub struct DocxImportResult {
    pub document: Value,
    pub warnings: Vec<String>,
}

const MAX_DOCX_FILE_BYTES: u64 = 512 * 1024 * 1024;
const MAX_DOCX_ARCHIVE_ENTRIES: usize = 10_000;
const MAX_DOCX_XML_BYTES: u64 = 16 * 1024 * 1024;
const MAX_DOCX_MEDIA_BYTES: u64 = 32 * 1024 * 1024;
const MAX_DOCX_TOTAL_MEDIA_BYTES: u64 = 256 * 1024 * 1024;
const MAX_DOCX_FOOTNOTE_TEXT_CHARS: usize = 2_000;

static BOOKMARK_ID_COUNTER: AtomicUsize = AtomicUsize::new(1);

/// True when the table's first row contains only `table_header` cells — the
/// editor's header-row convention (prosemirror-tables toggleHeaderRow).
fn first_row_is_header(table: &Value) -> bool {
    let Some(first_row) = table
        .get("content")
        .and_then(Value::as_array)
        .and_then(|rows| rows.first())
    else {
        return false;
    };
    let Some(cells) = first_row.get("content").and_then(Value::as_array) else {
        return false;
    };
    !cells.is_empty()
        && cells
            .iter()
            .all(|cell| cell.get("type").and_then(Value::as_str) == Some("table_header"))
}

fn read_docx_xml(
    archive: &mut zip::ZipArchive<std::fs::File>,
    name: &str,
) -> Result<String, ExportError> {
    let mut entry = archive
        .by_name(name)
        .map_err(|error| ExportError::Docx(error.to_string()))?;
    if entry.size() > MAX_DOCX_XML_BYTES {
        return Err(ExportError::Docx(format!(
            "DOCX XML entry {name} exceeds the {MAX_DOCX_XML_BYTES}-byte safety limit"
        )));
    }
    let mut bytes = Vec::with_capacity(entry.size() as usize);
    entry.read_to_end(&mut bytes)?;
    String::from_utf8(bytes).map_err(|error| ExportError::Docx(error.to_string()))
}

fn css_hex_to_word_highlight(color: &str) -> String {
    let c = color.trim().to_lowercase();
    if !c.starts_with('#') {
        return color.to_string();
    }
    match c.as_str() {
        "#ffff00" | "#fef08a" | "#ffff66" | "#ffff99" => "yellow".to_string(),
        "#00ff00" | "#92d050" | "#00b050" | "#008000" => "green".to_string(),
        "#00ffff" | "#00b0f0" => "cyan".to_string(),
        "#ff00ff" => "magenta".to_string(),
        "#ff0000" | "#c00000" => "red".to_string(),
        "#0000ff" | "#0070c0" | "#0563c1" => "blue".to_string(),
        "#000080" | "#002060" => "darkBlue".to_string(),
        "#008080" | "#004b50" => "darkCyan".to_string(),
        "#006400" => "darkGreen".to_string(),
        "#800080" | "#7030a0" => "darkMagenta".to_string(),
        "#800000" => "darkRed".to_string(),
        "#808000" | "#ffc000" => "darkYellow".to_string(),
        "#a9a9a9" | "#808080" | "#7f7f7f" => "darkGray".to_string(),
        "#d3d3d3" | "#c0c0c0" | "#bfbfbf" => "lightGray".to_string(),
        "#000000" => "black".to_string(),
        "#ffffff" => "white".to_string(),
        _ => "yellow".to_string(),
    }
}

fn word_highlight_to_css(name: &str) -> String {
    match name.to_lowercase().as_str() {
        "yellow" | "darkyellow" => "#ffff00".to_string(),
        "green" | "darkgreen" => "#00ff00".to_string(),
        "cyan" | "darkcyan" => "#00ffff".to_string(),
        "magenta" | "darkmagenta" => "#ff00ff".to_string(),
        "red" | "darkred" => "#ff0000".to_string(),
        "blue" | "darkblue" => "#0000ff".to_string(),
        "darkgray" => "#a9a9a9".to_string(),
        "lightgray" => "#d3d3d3".to_string(),
        "black" => "#000000".to_string(),
        "white" => "#ffffff".to_string(),
        other if other.starts_with('#') => other.to_string(),
        _ => "#ffff00".to_string(),
    }
}

fn add_list_numbering(docx: Docx) -> Docx {
    let mut bullet = AbstractNumbering::new(1);
    let mut ordered = AbstractNumbering::new(2);
    for lvl in 0..9 {
        bullet = bullet.add_level(
            Level::new(
                lvl,
                Start::new(1),
                NumberFormat::new("bullet"),
                LevelText::new("●"),
                LevelJc::new("left"),
            )
            .indent(
                Some(720 + lvl as i32 * 360),
                Some(SpecialIndentType::Hanging(360)),
                None,
                None,
            ),
        );
        ordered = ordered.add_level(
            Level::new(
                lvl,
                Start::new(1),
                NumberFormat::new("decimal"),
                LevelText::new(format!("%{}.", lvl + 1)),
                LevelJc::new("left"),
            )
            .indent(
                Some(720 + lvl as i32 * 360),
                Some(SpecialIndentType::Hanging(360)),
                None,
                None,
            ),
        );
    }
    docx.add_abstract_numbering(bullet)
        .add_abstract_numbering(ordered)
        .add_numbering(Numbering::new(1, 1))
        .add_numbering(Numbering::new(2, 2))
}

/// Numbering id 3..N: one concrete numbering per distinct ordered-list start
/// value, overriding level 0 to begin at `start`. Ids 1/2 stay bullet/ordered
/// at start=1.
/// Collect ordered-list start values (attrs.order) and assign a concrete
/// numbering id per distinct start. start=1 keeps the shared numbering 2.
/// Returns (start -> num_id map, next free id).
fn collect_ordered_list_starts(node: &Value, starts: &mut BTreeMap<u32, u32>, next_id: &mut u32) {
    if node.get("type").and_then(Value::as_str) == Some("ordered_list") {
        let start = node
            .get("attrs")
            .and_then(|attrs| attrs.get("order"))
            .and_then(Value::as_u64)
            .unwrap_or(1)
            .clamp(1, 999) as u32;
        if start > 1 && !starts.contains_key(&start) {
            starts.insert(start, *next_id);
            *next_id += 1;
        }
    }
    if let Some(children) = node.get("content").and_then(Value::as_array) {
        for child in children {
            collect_ordered_list_starts(child, starts, next_id);
        }
    }
}

fn add_list_start_numbering(docx: Docx, starts: &BTreeMap<u32, u32>) -> Docx {
    let mut docx = docx;
    for (start, num_id) in starts {
        let mut numbering = Numbering::new(*num_id as usize, 2);
        if *start > 1 {
            numbering = numbering.add_override(LevelOverride::new(0).start(*start as usize));
        }
        docx = docx.add_numbering(numbering);
    }
    docx
}

#[derive(Clone, Copy)]
struct DocxPageSetup {
    width: u32,
    height: u32,
    orientation: PageOrientationType,
    columns: usize,
    top: i32,
    bottom: i32,
    left: i32,
    right: i32,
}

fn json_number(value: Option<&Value>, default: f64) -> f64 {
    value
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite())
        .unwrap_or(default)
        .clamp(0.0, 12.0)
}

fn page_setup_value(doc_json: &Value) -> Option<&Value> {
    doc_json
        .get("pageSetup")
        .or_else(|| doc_json.get("page_setup"))
        .or_else(|| {
            doc_json
                .get("attrs")
                .and_then(|attrs| attrs.get("pageSetup").or_else(|| attrs.get("page_setup")))
        })
        .filter(|value| value.is_object())
}

fn paper_dimensions_twips(name: &str) -> (u32, u32) {
    match name {
        "a4" => (11906, 16838),
        "legal" => (12240, 20160),
        "executive" => (10440, 15120),
        _ => (12240, 15840),
    }
}

fn twips_from_inches(value: f64) -> i32 {
    (value * 1440.0).round().clamp(0.0, i32::MAX as f64) as i32
}

fn parse_export_page_setup(doc_json: &Value) -> Option<DocxPageSetup> {
    let setup = page_setup_value(doc_json)?;
    let paper_size = setup
        .get("paperSize")
        .or_else(|| setup.get("paper_size"))
        .and_then(Value::as_str)
        .unwrap_or("letter")
        .to_ascii_lowercase();
    let orientation = match setup
        .get("orientation")
        .and_then(Value::as_str)
        .unwrap_or("portrait")
        .to_ascii_lowercase()
        .as_str()
    {
        "landscape" => PageOrientationType::Landscape,
        _ => PageOrientationType::Portrait,
    };
    let (mut width, mut height) = paper_dimensions_twips(&paper_size);
    if orientation == PageOrientationType::Landscape {
        std::mem::swap(&mut width, &mut height);
    }
    let margins = setup.get("margins");
    let columns = setup
        .get("columns")
        .and_then(Value::as_u64)
        .unwrap_or(1)
        .clamp(1, 8) as usize;
    Some(DocxPageSetup {
        width,
        height,
        orientation,
        columns,
        top: twips_from_inches(json_number(margins.and_then(|value| value.get("top")), 1.0)),
        bottom: twips_from_inches(json_number(
            margins.and_then(|value| value.get("bottom")),
            1.0,
        )),
        left: twips_from_inches(json_number(
            margins.and_then(|value| value.get("left")),
            1.0,
        )),
        right: twips_from_inches(json_number(
            margins.and_then(|value| value.get("right")),
            1.0,
        )),
    })
}

fn add_header_footer_text(mut paragraph: Paragraph, text: &str) -> Paragraph {
    let mut remaining = text.chars().take(16_384).collect::<String>();
    while !remaining.is_empty() {
        let placeholders = [
            ("{page}", "page"),
            ("{pages}", "pages"),
            ("{total}", "pages"),
        ];
        let next = placeholders
            .iter()
            .filter_map(|(token, kind)| remaining.find(token).map(|index| (index, *token, *kind)))
            .min_by_key(|(index, _, _)| *index);
        let Some((index, token, kind)) = next else {
            paragraph = paragraph.add_run(Run::new().add_text(remaining));
            break;
        };
        if index > 0 {
            paragraph = paragraph.add_run(Run::new().add_text(remaining[..index].to_string()));
        }
        paragraph = if kind == "page" {
            paragraph.add_page_num(PageNum::new())
        } else {
            paragraph.add_num_pages(NumPages::new())
        };
        remaining = remaining[index + token.len()..].to_string();
    }
    paragraph
}

fn apply_export_page_setup(mut docx: Docx, doc_json: &Value) -> Docx {
    let Some(setup) = parse_export_page_setup(doc_json) else {
        return docx;
    };
    docx = docx
        .page_size(setup.width, setup.height)
        .page_orient(setup.orientation)
        .page_margin(PageMargin {
            top: setup.top,
            left: setup.left,
            bottom: setup.bottom,
            right: setup.right,
            header: 720,
            footer: 720,
            gutter: 0,
        });
    docx.document = docx.document.columns(setup.columns);

    if let Some(header) = page_setup_value(doc_json)
        .and_then(|value| value.get("header"))
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
    {
        let header = Header::new().add_paragraph(add_header_footer_text(Paragraph::new(), header));
        docx = docx.header(header);
    }
    if let Some(footer) = page_setup_value(doc_json)
        .and_then(|value| value.get("footer"))
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
    {
        let footer = Footer::new().add_paragraph(add_header_footer_text(Paragraph::new(), footer));
        docx = docx.footer(footer);
    }
    docx
}

struct RunImportState {
    bold: bool,
    italic: bool,
    underline: bool,
    strike: bool,
    color: Option<String>,
    highlight: Option<String>,
    font_size: Option<String>,
    font_family: Option<String>,
    superscript: bool,
    subscript: bool,
    link_href: Option<String>,
}

const MAX_DOC_HYPERLINK_CHARS: usize = 2_048;

fn safe_doc_hyperlink(target: &str) -> Option<String> {
    let target = target.trim();
    if target.is_empty()
        || target.chars().count() > MAX_DOC_HYPERLINK_CHARS
        || target.chars().any(char::is_control)
    {
        return None;
    }
    let lower = target.to_ascii_lowercase();
    if let Some(name) = lower
        .strip_prefix("internal:")
        .map(|_| target["internal:".len()..].trim())
        .or_else(|| target.strip_prefix('#').map(str::trim))
    {
        return if name.is_empty() {
            None
        } else {
            Some(format!("internal:{name}"))
        };
    }
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
        return Some(target.to_string());
    }
    None
}

#[derive(Clone)]
struct TrackedChangeContext {
    kind: &'static str,
    author: String,
    change_id: String,
    created_at: String,
}

impl RunImportState {
    fn new() -> Self {
        Self {
            bold: false,
            italic: false,
            underline: false,
            strike: false,
            color: None,
            highlight: None,
            font_size: None,
            font_family: None,
            superscript: false,
            subscript: false,
            link_href: None,
        }
    }

    fn clear(&mut self) {
        *self = RunImportState::new();
    }

    fn to_marks(
        &self,
        hyperlink: Option<&str>,
        tracked_change: Option<&TrackedChangeContext>,
    ) -> Vec<Value> {
        let mut marks = Vec::new();
        if self.bold {
            marks.push(json!({ "type": "bold" }));
        }
        if self.italic {
            marks.push(json!({ "type": "italic" }));
        }
        if self.underline {
            marks.push(json!({ "type": "underline" }));
        }
        if self.strike {
            marks.push(json!({ "type": "strike" }));
        }
        if self.superscript {
            marks.push(json!({ "type": "superscript" }));
        }
        if self.subscript {
            marks.push(json!({ "type": "subscript" }));
        }
        if let Some(family) = &self.font_family {
            marks.push(json!({ "type": "fontFamily", "attrs": { "family": family } }));
        }
        if let Some(size) = &self.font_size {
            marks.push(json!({ "type": "fontSize", "attrs": { "size": size } }));
        }
        if let Some(color) = &self.color {
            marks.push(json!({ "type": "color", "attrs": { "color": color } }));
        }
        if let Some(color) = &self.highlight {
            marks.push(json!({ "type": "highlight", "attrs": { "color": color } }));
        }
        if let Some(href) = hyperlink
            .or(self.link_href.as_deref())
            .and_then(safe_doc_hyperlink)
        {
            marks.push(json!({ "type": "link", "attrs": { "href": href, "title": null } }));
        }
        if let Some(change) = tracked_change {
            let mark_type = if change.kind == "insert" {
                "trackInsert"
            } else {
                "trackDelete"
            };
            marks.push(json!({
                "type": mark_type,
                "attrs": {
                    "author": change.author,
                    "changeId": change.change_id,
                    "createdAt": change.created_at,
                }
            }));
        }
        marks
    }
}

struct OpenList {
    list_type: String,
    num_id: u32,
    items: Vec<Value>,
    order: Option<u64>,
}

fn flush_open_list(open_list: &mut Option<OpenList>, paragraphs: &mut Vec<Value>) {
    if let Some(list) = open_list.take() {
        let order = (list.list_type == "ordered_list" && list.order.is_some_and(|o| o > 1))
            .then(|| json!({ "order": list.order }));
        let node = match order {
            Some(attrs) => json!({ "type": list.list_type, "attrs": attrs, "content": list.items }),
            None => json!({ "type": list.list_type, "content": list.items }),
        };
        paragraphs.push(node);
    }
}

fn list_type_for_num_id(num_id: u32, formats: &HashMap<u32, String>) -> String {
    match formats.get(&num_id).map(|s| s.as_str()) {
        Some("bullet") => "bullet_list".to_string(),
        Some("decimal") | Some("lowerLetter") | Some("upperLetter") | Some("lowerRoman")
        | Some("upperRoman") => "ordered_list".to_string(),
        None if num_id == 1 => "bullet_list".to_string(),
        None if num_id == 2 => "ordered_list".to_string(),
        _ => "ordered_list".to_string(),
    }
}

/// Close a w:fldChar field region: supported PAGE / NUMPAGES instructions
/// become field nodes; anything else is skipped with a warning. The cached
/// result (between separate and end) never leaks into the body.
#[allow(clippy::too_many_arguments)]
fn flush_field_node(
    content: &mut Vec<Value>,
    paragraph_text_units: &mut usize,
    active: bool,
    seen_instruction: bool,
    instruction: &str,
    warnings: &mut Vec<String>,
) {
    if !active || !seen_instruction {
        return;
    }
    let upper = instruction.to_ascii_uppercase();
    let kind = if upper.contains("NUMPAGES") {
        Some("numPages")
    } else if upper.split_whitespace().any(|token| token == "PAGE") {
        Some("page")
    } else {
        None
    };
    match kind {
        Some(kind) => {
            content.push(json!({
                "type": "field",
                "attrs": { "kind": kind, "result": null }
            }));
            *paragraph_text_units = paragraph_text_units.saturating_add(1);
        }
        None => record_unsupported_docx_construct(warnings, b"w:fldSimple"),
    }
}

fn record_unsupported_docx_construct(warnings: &mut Vec<String>, name: &[u8]) {
    let warning = match name {
        b"w:fldSimple" | b"w:instrText" => "Word fields were skipped.",
        b"w:br" => "Manual line breaks were simplified.",
        b"w:bookmarkStart" | b"w:bookmarkEnd" => "Bookmarks were skipped.",
        _ => return,
    };
    if !warnings.iter().any(|existing| existing == warning) {
        warnings.push(warning.to_string());
    }
}

#[derive(Clone, Default)]
struct ImportedDocxComment {
    author: String,
    date: String,
    text: String,
}

fn docx_local_name(name: &[u8]) -> &[u8] {
    name.rsplit(|byte| *byte == b':').next().unwrap_or(name)
}

fn docx_attr(element: &BytesStart<'_>, name: &[u8]) -> Option<String> {
    element.attributes().flatten().find_map(|attribute| {
        (docx_local_name(attribute.key.as_ref()) == name)
            .then(|| String::from_utf8_lossy(attribute.value.as_ref()).into_owned())
    })
}

fn docx_twips_to_inches(value: Option<&str>) -> Option<f64> {
    value
        .and_then(|value| value.parse::<f64>().ok())
        .filter(|value| value.is_finite())
        .map(|value| (value / 1440.0 * 1000.0).round() / 1000.0)
}

fn docx_paper_size(width: u32, height: u32) -> &'static str {
    let (width, height) = if width <= height {
        (width, height)
    } else {
        (height, width)
    };
    let candidates = [
        ("letter", 12240_u32, 15840_u32),
        ("a4", 11906, 16838),
        ("legal", 12240, 20160),
        ("executive", 10440, 15120),
    ];
    candidates
        .into_iter()
        .min_by_key(|(_, expected_width, expected_height)| {
            width.abs_diff(*expected_width) as u64 + height.abs_diff(*expected_height) as u64
        })
        .map(|(name, _, _)| name)
        .unwrap_or("letter")
}

fn docx_part_path(target: &str) -> Option<String> {
    let target = target.trim_start_matches('/');
    let target = target.strip_prefix("../").unwrap_or(target);
    let already_rooted = target.strip_prefix("word/");
    let relative = already_rooted.unwrap_or(target);
    if relative.is_empty()
        || relative
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return None;
    }
    Some(if already_rooted.is_some() {
        target.to_string()
    } else {
        format!("word/{target}")
    })
}

fn read_docx_story_text(
    archive: &mut zip::ZipArchive<std::fs::File>,
    part: &str,
) -> Result<Option<String>, ExportError> {
    let Ok(xml) = read_docx_xml(archive, part) else {
        return Ok(None);
    };
    let mut reader = Reader::from_str(&xml);
    reader.config_mut().trim_text(false);
    let mut buffer = Vec::new();
    let mut text = String::new();
    let mut in_text = false;
    let mut in_instruction = false;
    let mut field_result = false;
    let mut current_field = None::<&'static str>;
    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(event)) => match docx_local_name(event.name().as_ref()) {
                b"t" => in_text = true,
                b"instrText" => in_instruction = true,
                b"fldChar" => match docx_attr(&event, b"fldCharType").as_deref() {
                    Some("begin") => {
                        field_result = true;
                        current_field = None;
                    }
                    Some("separate") => field_result = true,
                    Some("end") => {
                        field_result = false;
                        current_field = None;
                    }
                    _ => {}
                },
                _ => {}
            },
            Ok(Event::Empty(event)) if docx_local_name(event.name().as_ref()) == b"fldChar" => {
                match docx_attr(&event, b"fldCharType").as_deref() {
                    Some("begin") => {
                        field_result = true;
                        current_field = None;
                    }
                    Some("separate") => field_result = true,
                    Some("end") => {
                        field_result = false;
                        current_field = None;
                    }
                    _ => {}
                }
            }
            Ok(Event::Text(event)) if in_text || in_instruction => {
                let value = event
                    .unescape()
                    .map_err(|error| ExportError::Docx(error.to_string()))?;
                if in_instruction {
                    let instruction = value.to_ascii_uppercase();
                    if instruction.contains("NUMPAGES") {
                        current_field = Some("{pages}");
                    } else if instruction.contains("PAGE") {
                        current_field = Some("{page}");
                    }
                    if let Some(field) = current_field {
                        text.push_str(field);
                    }
                } else if !field_result {
                    text.push_str(&value);
                }
            }
            Ok(Event::End(event)) => match docx_local_name(event.name().as_ref()) {
                b"t" => in_text = false,
                b"instrText" => in_instruction = false,
                b"p" if !text.ends_with('\n') => text.push('\n'),
                _ => {}
            },
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => return Err(ExportError::Docx(error.to_string())),
        }
        buffer.clear();
    }
    while text.ends_with('\n') {
        text.pop();
    }
    Ok(Some(text))
}

fn parse_docx_page_setup(
    document_xml: &str,
    relationships: &HashMap<String, (String, String)>,
    archive: &mut zip::ZipArchive<std::fs::File>,
    warnings: &mut Vec<String>,
) -> Result<Option<Value>, ExportError> {
    let mut reader = Reader::from_str(document_xml);
    reader.config_mut().trim_text(true);
    let mut buffer = Vec::new();
    let mut width = None::<u32>;
    let mut height = None::<u32>;
    let mut orientation = None::<String>;
    let mut margins = serde_json::Map::new();
    let mut columns = 1u32;
    let mut header_id = None::<String>;
    let mut footer_id = None::<String>;
    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(event)) | Ok(Event::Empty(event)) => {
                match docx_local_name(event.name().as_ref()) {
                    b"pgSz" => {
                        width = docx_attr(&event, b"w").and_then(|value| value.parse().ok());
                        height = docx_attr(&event, b"h").and_then(|value| value.parse().ok());
                        orientation = docx_attr(&event, b"orient");
                    }
                    b"pgMar" => {
                        for (name, key) in [
                            (b"top".as_slice(), "top"),
                            (b"bottom".as_slice(), "bottom"),
                            (b"left".as_slice(), "left"),
                            (b"right".as_slice(), "right"),
                        ] {
                            if let Some(value) =
                                docx_twips_to_inches(docx_attr(&event, name).as_deref())
                            {
                                margins.insert(key.to_string(), json!(value));
                            }
                        }
                    }
                    b"cols" => {
                        columns = docx_attr(&event, b"num")
                            .and_then(|value| value.parse::<u32>().ok())
                            .unwrap_or(1)
                            .clamp(1, 8);
                    }
                    b"headerReference" => {
                        if docx_attr(&event, b"type").as_deref().unwrap_or("default") == "default" {
                            header_id = docx_attr(&event, b"id");
                        }
                    }
                    b"footerReference"
                        if docx_attr(&event, b"type").as_deref().unwrap_or("default")
                            == "default" =>
                    {
                        footer_id = docx_attr(&event, b"id");
                    }
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => return Err(ExportError::Docx(error.to_string())),
        }
        buffer.clear();
    }
    let (Some(width), Some(height)) = (width, height) else {
        return Ok(None);
    };
    let orientation = orientation.unwrap_or_else(|| {
        if width > height {
            "landscape".to_string()
        } else {
            "portrait".to_string()
        }
    });
    let orientation = if orientation.eq_ignore_ascii_case("landscape") {
        "landscape"
    } else {
        "portrait"
    };
    let mut setup = serde_json::Map::new();
    setup.insert(
        "paperSize".to_string(),
        json!(docx_paper_size(width, height)),
    );
    setup.insert("orientation".to_string(), json!(orientation));
    setup.insert("columns".to_string(), json!(columns));
    if !margins.is_empty() {
        setup.insert("margins".to_string(), Value::Object(margins));
    }
    if let Some(id) = header_id.as_deref() {
        if let Some((target, relation_type)) = relationships.get(id) {
            if relation_type.contains("/header") {
                if let Some(part) = docx_part_path(target) {
                    if let Some(text) = read_docx_story_text(archive, &part)? {
                        if !text.is_empty() {
                            setup.insert("header".to_string(), json!(text));
                        }
                    }
                }
            }
        }
    }
    if let Some(id) = footer_id.as_deref() {
        if let Some((target, relation_type)) = relationships.get(id) {
            if relation_type.contains("/footer") {
                if let Some(part) = docx_part_path(target) {
                    if let Some(text) = read_docx_story_text(archive, &part)? {
                        if !text.is_empty() {
                            setup.insert("footer".to_string(), json!(text));
                        }
                    }
                }
            }
        }
    }
    if (header_id.is_some() && !setup.contains_key("header"))
        || (footer_id.is_some() && !setup.contains_key("footer"))
    {
        warnings.push("DOCX header/footer reference could not be read.".to_string());
    }
    Ok(Some(Value::Object(setup)))
}

fn read_docx_comments(
    archive: &mut zip::ZipArchive<std::fs::File>,
) -> Result<HashMap<usize, ImportedDocxComment>, ExportError> {
    let Ok(mut entry) = archive.by_name("word/comments.xml") else {
        return Ok(HashMap::new());
    };
    if entry.size() > MAX_DOCX_XML_BYTES {
        return Err(ExportError::Docx(format!(
            "DOCX comments entry exceeds the {MAX_DOCX_XML_BYTES}-byte safety limit"
        )));
    }
    let mut bytes = Vec::with_capacity(entry.size() as usize);
    entry.read_to_end(&mut bytes)?;
    let mut reader = Reader::from_reader(bytes.as_slice());
    reader.config_mut().trim_text(true);
    let mut buffer = Vec::new();
    let mut current = None::<(usize, ImportedDocxComment)>;
    let mut in_text = false;
    let mut comments = HashMap::new();
    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(element)) if docx_local_name(element.name().as_ref()) == b"comment" => {
                let Some(id) = docx_attr(&element, b"id").and_then(|value| value.parse().ok())
                else {
                    continue;
                };
                current = Some((
                    id,
                    ImportedDocxComment {
                        author: docx_attr(&element, b"author").unwrap_or_else(|| "Unknown".into()),
                        date: docx_attr(&element, b"date")
                            .unwrap_or_else(|| "1970-01-01T00:00:00Z".into()),
                        text: String::new(),
                    },
                ));
            }
            Ok(Event::Start(element))
                if current.is_some() && docx_local_name(element.name().as_ref()) == b"t" =>
            {
                in_text = true;
            }
            Ok(Event::Text(text)) if in_text => {
                if let Some((_, comment)) = current.as_mut() {
                    comment.text.push_str(
                        &text
                            .unescape()
                            .map_err(|error| ExportError::Docx(error.to_string()))?,
                    );
                }
            }
            Ok(Event::End(element)) if docx_local_name(element.name().as_ref()) == b"t" => {
                in_text = false;
            }
            Ok(Event::End(element)) if docx_local_name(element.name().as_ref()) == b"p" => {
                if let Some((_, comment)) = current.as_mut() {
                    if !comment.text.is_empty() {
                        comment.text.push('\n');
                    }
                }
            }
            Ok(Event::End(element)) if docx_local_name(element.name().as_ref()) == b"comment" => {
                if let Some((id, mut comment)) = current.take() {
                    while comment.text.ends_with('\n') {
                        comment.text.pop();
                    }
                    comments.insert(id, comment);
                }
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => return Err(ExportError::Docx(error.to_string())),
        }
        buffer.clear();
    }
    Ok(comments)
}

/// Parse `word/footnotes.xml` into word-id -> { id, label, text } entries.
/// Separator / continuation footnotes (ids 0 and 1) are skipped.
fn read_docx_footnotes(archive: &mut zip::ZipArchive<std::fs::File>) -> HashMap<u32, Value> {
    let Ok(mut entry) = archive.by_name("word/footnotes.xml") else {
        return HashMap::new();
    };
    if entry.size() > MAX_DOCX_XML_BYTES {
        return HashMap::new();
    }
    let mut bytes = Vec::new();
    if entry.read_to_end(&mut bytes).is_err() {
        return HashMap::new();
    }
    drop(entry);
    let mut reader = Reader::from_reader(bytes.as_slice());
    reader.config_mut().trim_text(true);
    let mut buffer = Vec::new();
    let mut current: Option<(u32, String)> = None; // (word id, text)
    let mut in_text = false;
    let mut label = 0u64;
    let mut notes: Vec<(u32, Value)> = Vec::new();
    while let Ok(event) = reader.read_event_into(&mut buffer) {
        match event {
            Event::Start(element) | Event::Empty(element)
                if docx_local_name(element.name().as_ref()) == b"footnote" =>
            {
                let id = docx_attr(&element, b"id")
                    .and_then(|value| value.parse::<u32>().ok())
                    .unwrap_or_default();
                // Separator notes carry w:type; real notes do not.
                let is_real = id >= 2 && docx_attr(&element, b"type").is_none();
                if is_real {
                    label += 1;
                    current = Some((id, String::new()));
                }
            }
            Event::Start(element)
                if current.is_some() && docx_local_name(element.name().as_ref()) == b"t" =>
            {
                in_text = true;
            }
            Event::Text(text) if in_text => {
                if let Ok(value) = text.unescape() {
                    if let Some((_, note_text)) = current.as_mut() {
                        note_text.push_str(&value);
                    }
                }
            }
            Event::End(element) if docx_local_name(element.name().as_ref()) == b"t" => {
                in_text = false;
            }
            Event::End(element) if docx_local_name(element.name().as_ref()) == b"footnote" => {
                if let Some((word_id, note_text)) = current.take() {
                    // Strip the leading superscript number Word writes into notes.
                    let text = note_text
                        .trim()
                        .trim_start_matches(|c: char| c.is_ascii_digit())
                        .trim()
                        .chars()
                        .take(MAX_DOCX_FOOTNOTE_TEXT_CHARS)
                        .collect::<String>();
                    notes.push((
                        word_id,
                        json!({
                            "id": format!("fn-docx-{word_id}"),
                            "label": label,
                            "text": text,
                        }),
                    ));
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buffer.clear();
    }
    notes.into_iter().collect()
}

fn build_run(child: &serde_json::Value, override_font: Option<&str>) -> (Run, Option<String>) {
    let mut link_url = None;
    let mut run = Run::new();
    let tracked_delete = child
        .get("marks")
        .and_then(Value::as_array)
        .is_some_and(|marks| {
            marks.iter().any(|mark| {
                mark.get("type")
                    .and_then(Value::as_str)
                    .map(redoc_doc_engine::normalize_mark_type)
                    == Some("trackDelete")
            })
        });

    if let Some(text) = child.get("text").and_then(|t| t.as_str()) {
        run = if tracked_delete {
            run.add_delete_text(text)
        } else {
            run.add_text(text)
        };
    }

    if let Some(font) = override_font {
        run = run.fonts(RunFonts::new().ascii(font));
    }

    if let Some(marks) = child.get("marks").and_then(|marks| marks.as_array()) {
        for mark in marks {
            let kind = mark
                .get("type")
                .and_then(|kind| kind.as_str())
                .map(redoc_doc_engine::normalize_mark_type);
            match kind {
                Some("bold") => run = run.bold(),
                Some("italic") => run = run.italic(),
                Some("underline") => run = run.underline("single"),
                Some("strike") => run = run.strike(),
                Some("superscript") => {
                    run.run_property = run.run_property.vert_align(VertAlignType::SuperScript);
                }
                Some("subscript") => {
                    run.run_property = run.run_property.vert_align(VertAlignType::SubScript);
                }
                Some("fontFamily") => {
                    if override_font.is_none() {
                        if let Some(family) = mark
                            .get("attrs")
                            .and_then(|attrs| attrs.get("family"))
                            .and_then(|value| value.as_str())
                        {
                            run = run.fonts(RunFonts::new().ascii(family));
                        }
                    }
                }
                Some("fontSize") => {
                    if let Some(size) = mark
                        .get("attrs")
                        .and_then(|attrs| attrs.get("size"))
                        .and_then(|value| value.as_f64().or_else(|| value.as_str()?.parse().ok()))
                    {
                        run = run.size((size * 2.0) as usize);
                    }
                }
                Some("color") => {
                    if let Some(color) = mark
                        .get("attrs")
                        .and_then(|attrs| attrs.get("color"))
                        .and_then(|value| value.as_str())
                    {
                        run = run.color(color.trim_start_matches('#'));
                    }
                }
                Some("highlight") => {
                    let color = mark
                        .get("attrs")
                        .and_then(|a| a.get("color").or_else(|| a.get("val")))
                        .and_then(|v| v.as_str())
                        .unwrap_or("yellow");
                    run = run.highlight(css_hex_to_word_highlight(color));
                }
                Some("link") => {
                    let href = mark
                        .get("attrs")
                        .and_then(|a| a.get("href"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("#");
                    link_url = Some(href.to_string());
                    run = run.color("0563C1").underline("single");
                }
                Some("trackInsert") => {
                    run = run.color("008000").underline("single");
                }
                Some("trackDelete") => {
                    run = run.color("C00000").strike();
                }
                _ => {}
            }
        }
    }

    (run, link_url)
}

fn tracked_change_metadata(child: &Value, kind: &str) -> Option<(String, String)> {
    child.get("marks")?.as_array()?.iter().find_map(|mark| {
        let mark_type = mark
            .get("type")
            .and_then(Value::as_str)
            .map(redoc_doc_engine::normalize_mark_type)?;
        if mark_type != kind {
            return None;
        }
        let attrs = mark.get("attrs");
        let author = attrs
            .and_then(|attrs| attrs.get("author"))
            .and_then(Value::as_str)
            .unwrap_or("Unknown")
            .to_string();
        let date = attrs
            .and_then(|attrs| attrs.get("createdAt"))
            .and_then(Value::as_str)
            .unwrap_or("1970-01-01T00:00:00Z")
            .to_string();
        Some((author, date))
    })
}

fn next_bookmark_id() -> usize {
    BOOKMARK_ID_COUNTER.fetch_add(1, Ordering::Relaxed)
}

fn bookmark_names(child: &Value) -> Vec<String> {
    let mut names = Vec::new();
    if let Some(marks) = child.get("marks").and_then(Value::as_array) {
        for mark in marks {
            if mark.get("type").and_then(Value::as_str) == Some("bookmark") {
                if let Some(name) = mark
                    .get("attrs")
                    .and_then(|attrs| attrs.get("name"))
                    .and_then(Value::as_str)
                {
                    let name = name.trim();
                    if !name.is_empty() && !names.iter().any(|existing| existing == name) {
                        names.push(name.to_string());
                    }
                }
            }
        }
    }
    names
}

fn add_runs_to_paragraph(
    p: Paragraph,
    child: &serde_json::Value,
    override_font: Option<&str>,
) -> Paragraph {
    if child.get("type").and_then(Value::as_str) == Some("image") {
        if let Some(src) = child
            .get("attrs")
            .and_then(|attrs| attrs.get("src"))
            .and_then(Value::as_str)
        {
            if let Some(bytes) = decode_data_uri(src) {
                let attr_px = |key: &str, fallback: u32| -> u32 {
                    child
                        .get("attrs")
                        .and_then(|attrs| attrs.get(key))
                        .and_then(Value::as_f64)
                        .map(|v| v.clamp(1.0, 10_000.0) as u32)
                        .unwrap_or(fallback)
                };
                let width = attr_px("width", 320).max(1);
                let height = attr_px("height", 240).max(1);
                return p
                    .add_run(Run::new().add_image(Pic::new_with_dimensions(bytes, width, height)));
            }
        }
        return p;
    }

    // Footnote reference: emit a marker run that the post-pack surgery
    // rewrites into a native <w:footnoteReference> with the footnotes part.
    if child.get("type").and_then(Value::as_str) == Some("footnote_ref") {
        let attrs = child.get("attrs");
        let id = attrs
            .and_then(|attrs| attrs.get("id"))
            .and_then(Value::as_str)
            .unwrap_or_default();
        let label = attrs
            .and_then(|attrs| attrs.get("label"))
            .map(|value| value.to_string().replace('"', ""))
            .unwrap_or_default();
        let mut run = Run::new().add_text(format!("[[FN:{id}|{label}]]"));
        run.run_property = run.run_property.vert_align(VertAlignType::SuperScript);
        return p.add_run(run);
    }

    // Inline document field: native PAGE / NUMPAGES field code, matching the
    // header/footer token pattern so Word renders and recalculates them.
    if child.get("type").and_then(Value::as_str) == Some("field") {
        let kind = child
            .get("attrs")
            .and_then(|attrs| attrs.get("kind"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_ascii_lowercase();
        return match kind.as_str() {
            "numpages" => p.add_num_pages(NumPages::new()),
            _ => p.add_page_num(PageNum::new()),
        };
    }

    let Some(text) = child.get("text").and_then(|t| t.as_str()) else {
        return p;
    };

    let bookmarks = bookmark_names(child);
    let mut p = p;
    let bookmark_ids: Vec<(usize, String)> = bookmarks
        .iter()
        .map(|name| (next_bookmark_id(), name.clone()))
        .collect();
    for (id, name) in &bookmark_ids {
        p = p.add_bookmark_start(*id, name.clone());
    }
    let lines: Vec<&str> = text.split('\n').collect();
    for (idx, line) in lines.iter().enumerate() {
        let dummy_node = json!({
            "type": "text",
            "text": line,
            "marks": child.get("marks").cloned().unwrap_or(Value::Null)
        });
        let (mut run, link_url) = build_run(&dummy_node, override_font);
        if idx > 0 {
            run = run.add_break(BreakType::TextWrapping);
        }
        if let Some(url) = link_url {
            let hyperlink = if let Some(anchor) = url
                .strip_prefix("internal:")
                .map(str::trim)
                .filter(|anchor| !anchor.is_empty())
            {
                Hyperlink::new(anchor, HyperlinkType::Anchor)
            } else if let Some(anchor) = url
                .strip_prefix('#')
                .map(str::trim)
                .filter(|anchor| !anchor.is_empty())
            {
                Hyperlink::new(anchor, HyperlinkType::Anchor)
            } else {
                Hyperlink::new(url, HyperlinkType::External)
            };
            p = p.add_hyperlink(hyperlink.add_run(run));
        } else if let Some((author, date)) = tracked_change_metadata(child, "trackInsert") {
            p = p.add_insert(Insert::new(run).author(author).date(date));
        } else if let Some((author, date)) = tracked_change_metadata(child, "trackDelete") {
            p = p.add_delete(Delete::new().author(author).date(date).add_run(run));
        } else {
            p = p.add_run(run);
        }
    }
    for (id, _) in bookmark_ids.iter().rev() {
        p = p.add_bookmark_end(*id);
    }
    p
}

#[derive(Clone)]
struct ExportCommentAnchor {
    id: usize,
    from: usize,
    to: usize,
    comment: Comment,
}

#[derive(Clone, Default)]
struct ParagraphCommentLayout {
    start: usize,
    anchors: Vec<ExportCommentAnchor>,
}

fn utf16_len(value: &str) -> usize {
    value.encode_utf16().count()
}

fn utf16_byte_offset(value: &str, units: usize) -> usize {
    if units == 0 {
        return 0;
    }
    let mut consumed = 0usize;
    for (offset, character) in value.char_indices() {
        let width = character.len_utf16();
        if consumed.saturating_add(width) > units {
            return offset;
        }
        consumed = consumed.saturating_add(width);
        if consumed == units {
            return offset + character.len_utf8();
        }
    }
    value.len()
}

fn doc_node_size(node: &Value) -> usize {
    if let Some(text) = node.get("text").and_then(Value::as_str) {
        return utf16_len(text);
    }
    let Some(content) = node.get("content").and_then(Value::as_array) else {
        return 1;
    };
    let content_size = content.iter().map(doc_node_size).sum::<usize>();
    if node.get("type").and_then(Value::as_str) == Some("doc") {
        content_size
    } else {
        content_size.saturating_add(2)
    }
}

fn collect_text_spans(node: &Value, start: usize, spans: &mut Vec<(usize, usize)>) {
    if let Some(text) = node.get("text").and_then(Value::as_str) {
        let end = start.saturating_add(utf16_len(text));
        if end > start {
            spans.push((start, end));
        }
        return;
    }
    let Some(content) = node.get("content").and_then(Value::as_array) else {
        return;
    };
    let mut child_start = if node.get("type").and_then(Value::as_str) == Some("doc") {
        start
    } else {
        start.saturating_add(1)
    };
    for child in content {
        collect_text_spans(child, child_start, spans);
        child_start = child_start.saturating_add(doc_node_size(child));
    }
}

fn collect_paragraph_comment_layouts(
    node: &Value,
    start: usize,
    comments: &[ExportCommentAnchor],
    assigned: &mut std::collections::HashSet<usize>,
    layouts: &mut Vec<ParagraphCommentLayout>,
) {
    let node_type = node.get("type").and_then(Value::as_str);
    if matches!(
        node_type,
        Some("paragraph" | "heading" | "blockquote" | "code_block")
    ) {
        let mut spans = Vec::new();
        collect_text_spans(node, start, &mut spans);
        if let (Some((text_start, _)), Some((_, text_end))) = (spans.first(), spans.last()) {
            let mut anchors = Vec::new();
            for anchor in comments {
                if assigned.contains(&anchor.id)
                    || anchor.to <= *text_start
                    || anchor.from >= *text_end
                {
                    continue;
                }
                let from = anchor.from.max(*text_start);
                let to = anchor.to.min(*text_end);
                if from < to {
                    let mut mapped = anchor.clone();
                    mapped.from = from;
                    mapped.to = to;
                    anchors.push(mapped);
                    assigned.insert(anchor.id);
                }
            }
            anchors.sort_by_key(|anchor| (anchor.from, anchor.to, anchor.id));
            layouts.push(ParagraphCommentLayout { start, anchors });
        } else {
            layouts.push(ParagraphCommentLayout {
                start,
                anchors: Vec::new(),
            });
        }
    }

    let Some(content) = node.get("content").and_then(Value::as_array) else {
        return;
    };
    let mut child_start = if node_type == Some("doc") {
        start
    } else {
        start.saturating_add(1)
    };
    for child in content {
        collect_paragraph_comment_layouts(child, child_start, comments, assigned, layouts);
        child_start = child_start.saturating_add(doc_node_size(child));
    }
}

fn export_comment_anchors(doc_json: &Value) -> Vec<ExportCommentAnchor> {
    let Some(comments) = doc_json.get("comments").and_then(Value::as_array) else {
        return Vec::new();
    };
    let mut anchors = Vec::new();
    // Threaded replies export as separate w:comment entries referencing the
    // top-level comment through paraId-linked w15:commentEx parent ids.
    // Reply ids start above the top-level id range to stay unique.
    let mut next_id = comments.len();
    for (index, value) in comments.iter().enumerate() {
        let Some(from) = value.get("from").and_then(Value::as_u64) else {
            continue;
        };
        let Some(to) = value.get("to").and_then(Value::as_u64) else {
            continue;
        };
        if from >= to {
            continue;
        }
        let author = value
            .get("author")
            .and_then(Value::as_str)
            .unwrap_or("Unknown");
        let text = value.get("text").and_then(Value::as_str).unwrap_or("");
        let date = value
            .get("createdAt")
            .and_then(Value::as_str)
            .unwrap_or("1970-01-01T00:00:00Z");
        let id = index.saturating_add(1);
        anchors.push(ExportCommentAnchor {
            id,
            from: from as usize,
            to: to as usize,
            comment: Comment::new(id)
                .author(author)
                .date(date)
                .add_paragraph(Paragraph::new().add_run(Run::new().add_text(text))),
        });
        if let Some(replies) = value.get("replies").and_then(Value::as_array) {
            for reply in replies {
                let reply_author = reply
                    .get("author")
                    .and_then(Value::as_str)
                    .unwrap_or("Unknown");
                let reply_text = reply.get("text").and_then(Value::as_str).unwrap_or("");
                let reply_date = reply
                    .get("createdAt")
                    .and_then(Value::as_str)
                    .unwrap_or("1970-01-01T00:00:00Z");
                next_id = next_id.saturating_add(1);
                anchors.push(ExportCommentAnchor {
                    id: next_id,
                    from: from as usize,
                    to: to as usize,
                    comment: Comment::new(next_id)
                        .author(reply_author)
                        .date(reply_date)
                        .parent_comment_id(id)
                        .add_paragraph(Paragraph::new().add_run(Run::new().add_text(reply_text))),
                });
            }
        }
    }
    anchors
}

fn add_runs_to_paragraph_with_comments(
    mut paragraph: Paragraph,
    child: &Value,
    child_start: usize,
    anchors: &[ExportCommentAnchor],
    started: &mut std::collections::HashSet<usize>,
    ended: &mut std::collections::HashSet<usize>,
    override_font: Option<&str>,
) -> Paragraph {
    let Some(text) = child.get("text").and_then(Value::as_str) else {
        return add_runs_to_paragraph(paragraph, child, override_font);
    };
    let child_end = child_start.saturating_add(utf16_len(text));
    for anchor in anchors {
        if !started.contains(&anchor.id) && anchor.from <= child_start && anchor.to > child_start {
            paragraph = paragraph.add_comment_start(anchor.comment.clone());
            started.insert(anchor.id);
        }
    }
    let mut boundaries = vec![child_start, child_end];
    for anchor in anchors {
        if anchor.from > child_start && anchor.from < child_end {
            boundaries.push(anchor.from);
        }
        if anchor.to > child_start && anchor.to < child_end {
            boundaries.push(anchor.to);
        }
    }
    boundaries.sort_unstable();
    boundaries.dedup();
    for window in boundaries.windows(2) {
        let segment_start = window[0];
        let segment_end = window[1];
        for anchor in anchors {
            if !started.contains(&anchor.id)
                && anchor.from <= segment_start
                && anchor.to > segment_start
            {
                paragraph = paragraph.add_comment_start(anchor.comment.clone());
                started.insert(anchor.id);
            }
        }
        if segment_end > segment_start {
            let local_start = segment_start.saturating_sub(child_start);
            let local_end = segment_end.saturating_sub(child_start);
            let byte_start = utf16_byte_offset(text, local_start);
            let byte_end = utf16_byte_offset(text, local_end);
            let mut segment = child.clone();
            segment["text"] = Value::String(text[byte_start..byte_end].to_string());
            paragraph = add_runs_to_paragraph(paragraph, &segment, override_font);
        }
        for anchor in anchors {
            if anchor.to == segment_end && started.contains(&anchor.id) {
                paragraph = paragraph.add_comment_end(anchor.id);
                ended.insert(anchor.id);
            }
        }
    }
    paragraph
}

pub fn export_doc_to_docx(
    doc_json: &serde_json::Value,
    _title: &str,
) -> Result<Vec<u8>, ExportError> {
    let mut ordered_starts: BTreeMap<u32, u32> = BTreeMap::new();
    {
        let mut next_id = 3u32;
        collect_ordered_list_starts(doc_json, &mut ordered_starts, &mut next_id);
    }
    let mut docx = add_list_start_numbering(add_list_numbering(Docx::new()), &ordered_starts);
    let mut header_row_tables: Vec<bool> = Vec::new();
    let comment_anchors = export_comment_anchors(doc_json);
    let mut comment_assigned = std::collections::HashSet::new();
    let mut paragraph_layouts = Vec::new();
    collect_paragraph_comment_layouts(
        doc_json,
        0,
        &comment_anchors,
        &mut comment_assigned,
        &mut paragraph_layouts,
    );
    let mut paragraph_index = 0usize;

    fn build_nodes(
        node: &serde_json::Value,
        docx: &mut Docx,
        list_info: Option<(u32, u32)>,
        paragraph_layouts: &[ParagraphCommentLayout],
        paragraph_index: &mut usize,
        ordered_starts: &BTreeMap<u32, u32>,
        header_row_tables: &mut Vec<bool>,
    ) {
        let node_type = node.get("type").and_then(|t| t.as_str());

        if node_type == Some("table") {
            // Word requires rectangular grids: track each cell's covered
            // grid width (colspan) and vertical span (rowspan) so covered
            // vMerge=Continue cells can be emitted for merged regions.
            struct PendingCell {
                cell: TableCell,
                colspan: usize,
                rowspan: u32,
            }
            let mut rows: Vec<Vec<PendingCell>> = Vec::new();
            let mut grid: Vec<usize> = Vec::new();
            if let Some(row_nodes) = node.get("content").and_then(|c| c.as_array()) {
                for row in row_nodes {
                    let mut cells: Vec<PendingCell> = Vec::new();
                    if let Some(cell_nodes) = row.get("content").and_then(|c| c.as_array()) {
                        let mut grid_cursor = 0usize;
                        for cell in cell_nodes {
                            let mut tc = TableCell::new();
                            let mut colspan = 1usize;
                            let mut rowspan = 1u32;
                            if let Some(attrs) = cell.get("attrs") {
                                if let Some(span) = attrs.get("colspan").and_then(|v| v.as_u64()) {
                                    if span > 1 {
                                        tc = tc.grid_span(span as usize);
                                        colspan = span as usize;
                                    }
                                }
                                if let Some(span) = attrs.get("rowspan").and_then(|v| v.as_u64()) {
                                    if span > 1 {
                                        tc = tc.vertical_merge(VMergeType::Restart);
                                        rowspan = span as u32;
                                    }
                                }
                                if let Some(vmerge) = attrs
                                    .get("vmerge")
                                    .or_else(|| attrs.get("vMerge"))
                                    .and_then(|v| v.as_str())
                                {
                                    match vmerge {
                                        "restart" => tc = tc.vertical_merge(VMergeType::Restart),
                                        "continue" => tc = tc.vertical_merge(VMergeType::Continue),
                                        _ => {}
                                    }
                                }
                                // columnResizing colwidth: per-covered-column
                                // pixel widths at 96dpi -> twips.
                                if let Some(widths) =
                                    attrs.get("colwidth").and_then(|v| v.as_array())
                                {
                                    for (offset, width) in widths.iter().enumerate() {
                                        if let Some(px) = width.as_f64() {
                                            let twips =
                                                (px * 15.0).round().clamp(1.0, 31_680.0) as usize;
                                            let index = grid_cursor + offset;
                                            if index < grid.len() {
                                                grid[index] = twips;
                                            } else {
                                                grid.resize(index + 1, 0);
                                                grid[index] = twips;
                                            }
                                        }
                                    }
                                }
                            }
                            grid_cursor += colspan;

                            let mut added_p = false;
                            if let Some(p_nodes) = cell.get("content").and_then(|c| c.as_array()) {
                                for p_node in p_nodes {
                                    let mut cell_p = Paragraph::new();
                                    let layout = paragraph_layouts
                                        .get(*paragraph_index)
                                        .cloned()
                                        .unwrap_or_default();
                                    *paragraph_index = (*paragraph_index).saturating_add(1);
                                    let mut started = std::collections::HashSet::new();
                                    let mut ended = std::collections::HashSet::new();
                                    let mut child_start = layout.start.saturating_add(1);
                                    if let Some(p_content) =
                                        p_node.get("content").and_then(|c| c.as_array())
                                    {
                                        for child in p_content {
                                            cell_p = add_runs_to_paragraph_with_comments(
                                                cell_p,
                                                child,
                                                child_start,
                                                &layout.anchors,
                                                &mut started,
                                                &mut ended,
                                                None,
                                            );
                                            child_start =
                                                child_start.saturating_add(doc_node_size(child));
                                        }
                                    }
                                    for anchor in &layout.anchors {
                                        if started.contains(&anchor.id)
                                            && !ended.contains(&anchor.id)
                                        {
                                            cell_p = cell_p.add_comment_end(anchor.id);
                                        }
                                    }
                                    tc = tc.add_paragraph(cell_p);
                                    added_p = true;
                                }
                            }
                            if !added_p {
                                tc = tc.add_paragraph(Paragraph::new());
                            }
                            cells.push(PendingCell {
                                cell: tc,
                                colspan,
                                rowspan,
                            });
                        }
                    }
                    rows.push(cells);
                }
            }
            let grid_width = grid.len();
            // Emit vMerge=Continue covered cells so every row covers the full
            // grid width. Covered cells are inserted in column order by
            // tracking each row's covered columns.
            for row_index in 0..rows.len() {
                // Pass 1 (read-only): this row's vertical-merge spans with the
                // grid columns they start at.
                let mut cursor = 0usize;
                let mut merge_spans: Vec<(usize, usize, u32)> = Vec::new();
                for cell in &rows[row_index] {
                    if cell.rowspan > 1 {
                        merge_spans.push((cursor, cell.colspan, cell.rowspan));
                    }
                    cursor += cell.colspan;
                }
                // Pass 2 (mutate): insert covered cells into later rows at
                // their column positions (cells arrive left-to-right).
                for (start_col, colspan, rowspan) in merge_spans {
                    for depth in 1..rowspan {
                        let target = row_index + depth as usize;
                        if target >= rows.len() {
                            continue;
                        }
                        let covered = PendingCell {
                            cell: TableCell::new()
                                .vertical_merge(VMergeType::Continue)
                                .add_paragraph(Paragraph::new()),
                            colspan,
                            rowspan: 1,
                        };
                        let row = &mut rows[target];
                        let mut insert_at = 0usize;
                        let mut col = 0usize;
                        for cell in row.iter() {
                            if col >= start_col {
                                break;
                            }
                            col += cell.colspan;
                            insert_at += 1;
                        }
                        row.insert(insert_at, covered);
                    }
                }
                // Pad short rows so the grid stays rectangular in Word.
                let width: usize = rows[row_index].iter().map(|c| c.colspan).sum();
                if width < grid_width {
                    let mut row = std::mem::take(&mut rows[row_index]);
                    for _ in width..grid_width {
                        row.push(PendingCell {
                            cell: TableCell::new()
                                .vertical_merge(VMergeType::Continue)
                                .add_paragraph(Paragraph::new()),
                            colspan: 1,
                            rowspan: 1,
                        });
                    }
                    rows[row_index] = row;
                }
            }
            let is_header_table = rows.first().is_some_and(|_| first_row_is_header(node));
            let mut table = Table::new(
                rows.into_iter()
                    .map(|cells| TableRow::new(cells.into_iter().map(|c| c.cell).collect()))
                    .collect(),
            )
            .set_borders(TableBorders::new());
            if !grid.is_empty() {
                table = table.set_grid(grid);
            }
            *docx = docx.clone().add_table(table);
            header_row_tables.push(is_header_table);
            return;
        }

        if node_type == Some("page_break") {
            *docx = docx
                .clone()
                .add_paragraph(Paragraph::new().add_run(Run::new().add_break(BreakType::Page)));
            return;
        }

        if node_type == Some("section_break") {
            // Emit a real w:sectPr section break carrying the embedded page
            // setup instead of silently dropping the node. docx-rs renders an
            // empty Section as a paragraph whose pPr holds the sectPr.
            let mut section = Section::new();
            let setup_object = node
                .get("attrs")
                .and_then(|attrs| attrs.get("pageSetup"))
                .and_then(Value::as_object)
                .cloned()
                .map(serde_json::Value::Object);
            if let Some(setup) = setup_object {
                let wrapper = serde_json::json!({ "pageSetup": setup });
                if let Some(parsed) = parse_export_page_setup(&wrapper) {
                    section = section
                        .page_size(PageSize::new().width(parsed.width).height(parsed.height))
                        .page_orient(parsed.orientation)
                        .page_margin(PageMargin {
                            top: parsed.top,
                            left: parsed.left,
                            bottom: parsed.bottom,
                            right: parsed.right,
                            header: 720,
                            footer: 720,
                            gutter: 0,
                        });
                }
            }
            *docx = docx.clone().add_section(section);
            return;
        }

        if node_type == Some("code_block") {
            let mut p = Paragraph::new();
            let layout = paragraph_layouts
                .get(*paragraph_index)
                .cloned()
                .unwrap_or_default();
            *paragraph_index = (*paragraph_index).saturating_add(1);
            let mut started = std::collections::HashSet::new();
            let mut ended = std::collections::HashSet::new();
            let mut child_start = layout.start.saturating_add(1);
            p.property = p.property.shading(Shading::new().fill("F4F4F4"));
            if let Some(content) = node.get("content").and_then(|c| c.as_array()) {
                for child in content {
                    p = add_runs_to_paragraph_with_comments(
                        p,
                        child,
                        child_start,
                        &layout.anchors,
                        &mut started,
                        &mut ended,
                        Some("Courier New"),
                    );
                    child_start = child_start.saturating_add(doc_node_size(child));
                }
            }
            for anchor in &layout.anchors {
                if started.contains(&anchor.id) && !ended.contains(&anchor.id) {
                    p = p.add_comment_end(anchor.id);
                }
            }
            *docx = docx.clone().add_paragraph(p);
            return;
        }

        if node_type == Some("bullet_list") {
            let level = list_info.map(|(_, lvl)| lvl + 1).unwrap_or(0);
            if let Some(content) = node.get("content").and_then(|c| c.as_array()) {
                for child in content {
                    build_nodes(
                        child,
                        docx,
                        Some((1, level)),
                        paragraph_layouts,
                        paragraph_index,
                        ordered_starts,
                        header_row_tables,
                    );
                }
            }
            return;
        }

        if node_type == Some("ordered_list") {
            let level = list_info.map(|(_, lvl)| lvl + 1).unwrap_or(0);
            // Ordered lists starting past 1 use their own numbering instance
            // (level-0 start override) so "10." renders as 10.
            let num_id = node
                .get("attrs")
                .and_then(|attrs| attrs.get("order"))
                .and_then(Value::as_u64)
                .filter(|order| *order > 1)
                .and_then(|order| ordered_starts.get(&(order.clamp(1, 999) as u32)))
                .copied()
                .unwrap_or(2);
            if let Some(content) = node.get("content").and_then(|c| c.as_array()) {
                for child in content {
                    build_nodes(
                        child,
                        docx,
                        Some((num_id, level)),
                        paragraph_layouts,
                        paragraph_index,
                        ordered_starts,
                        header_row_tables,
                    );
                }
            }
            return;
        }

        if node_type == Some("list_item") {
            if let Some(content) = node.get("content").and_then(|c| c.as_array()) {
                for child in content {
                    build_nodes(
                        child,
                        docx,
                        list_info,
                        paragraph_layouts,
                        paragraph_index,
                        ordered_starts,
                        header_row_tables,
                    );
                }
            }
            return;
        }

        if node_type == Some("blockquote") {
            if let Some(content) = node.get("content").and_then(|c| c.as_array()) {
                let has_blocks = content
                    .iter()
                    .any(|child| child.get("type").and_then(Value::as_str) == Some("paragraph"));
                if has_blocks {
                    for child in content {
                        build_nodes(
                            child,
                            docx,
                            list_info,
                            paragraph_layouts,
                            paragraph_index,
                            ordered_starts,
                            header_row_tables,
                        );
                    }
                    return;
                }
            }
        }

        if let Some("paragraph" | "heading" | "blockquote") = node_type {
            let mut p = Paragraph::new();
            let layout = paragraph_layouts
                .get(*paragraph_index)
                .cloned()
                .unwrap_or_default();
            *paragraph_index = (*paragraph_index).saturating_add(1);
            let mut started = std::collections::HashSet::new();
            let mut ended = std::collections::HashSet::new();
            let mut child_start = layout.start.saturating_add(1);

            if let Some((num_id, level)) = list_info {
                p = p.numbering(
                    NumberingId::new(num_id as usize),
                    IndentLevel::new(level as usize),
                );
            }

            if node_type == Some("heading") {
                let level = node
                    .get("attrs")
                    .and_then(|attrs| attrs.get("level"))
                    .and_then(|level| level.as_u64())
                    .unwrap_or(1);
                p = p.style(match level {
                    2 => "Heading2",
                    3 => "Heading3",
                    4 => "Heading4",
                    5 => "Heading5",
                    6 => "Heading6",
                    _ => "Heading1",
                });
            } else {
                // Named paragraph style: recognized built-ins map to Word
                // styles; custom names round-trip verbatim as pStyle ids.
                let style_name = node
                    .get("attrs")
                    .and_then(|attrs| attrs.get("styleName"))
                    .and_then(|value| value.as_str())
                    .unwrap_or_default();
                if !style_name.is_empty() && style_name != "Normal" {
                    p = p.style(style_name);
                }
            }

            if let Some(attrs) = node.get("attrs") {
                p = match attrs.get("align").and_then(|value| value.as_str()) {
                    Some("center") => p.align(AlignmentType::Center),
                    Some("right") => p.align(AlignmentType::Right),
                    Some("justify") => p.align(AlignmentType::Both),
                    _ => p,
                };
                if let Some(indent) = attrs.get("indent").and_then(|value| value.as_f64()) {
                    p = p.indent(Some((indent * 720.0) as i32), None, None, None);
                }
                // One combined w:spacing element: line spacing + paragraph
                // spacing before/after (pt -> twips), all bounded.
                let line_height = attrs.get("lineHeight").and_then(|value| value.as_f64());
                let spacing_before = attrs
                    .get("spacingBefore")
                    .and_then(|value| value.as_f64())
                    .unwrap_or(0.0);
                let spacing_after = attrs
                    .get("spacingAfter")
                    .and_then(|value| value.as_f64())
                    .unwrap_or(0.0);
                if line_height.is_some() || spacing_before > 0.0 || spacing_after > 0.0 {
                    let mut spacing = LineSpacing::new();
                    if let Some(line_height) = line_height {
                        spacing = spacing
                            .line((line_height.clamp(0.5, 10.0) * 240.0) as i32)
                            .line_rule(LineSpacingType::Auto);
                    }
                    if spacing_before > 0.0 {
                        spacing = spacing
                            .before((spacing_before.clamp(0.0, 3168.0) * 20.0).round() as u32);
                    }
                    if spacing_after > 0.0 {
                        spacing =
                            spacing.after((spacing_after.clamp(0.0, 3168.0) * 20.0).round() as u32);
                    }
                    p = p.line_spacing(spacing);
                }
            }

            if let Some(content) = node.get("content").and_then(|c| c.as_array()) {
                for child in content {
                    p = add_runs_to_paragraph_with_comments(
                        p,
                        child,
                        child_start,
                        &layout.anchors,
                        &mut started,
                        &mut ended,
                        None,
                    );
                    child_start = child_start.saturating_add(doc_node_size(child));
                }
            }
            for anchor in &layout.anchors {
                if started.contains(&anchor.id) && !ended.contains(&anchor.id) {
                    p = p.add_comment_end(anchor.id);
                }
            }
            *docx = docx.clone().add_paragraph(p);
            return;
        }

        if let Some(content) = node.get("content").and_then(|c| c.as_array()) {
            for child in content {
                build_nodes(
                    child,
                    docx,
                    list_info,
                    paragraph_layouts,
                    paragraph_index,
                    ordered_starts,
                    header_row_tables,
                );
            }
        }
    }

    build_nodes(
        doc_json,
        &mut docx,
        None,
        &paragraph_layouts,
        &mut paragraph_index,
        &ordered_starts,
        &mut header_row_tables,
    );

    docx = apply_export_page_setup(docx, doc_json);

    let mut buf = Vec::new();
    docx.build()
        .pack(std::io::Cursor::new(&mut buf))
        .map_err(|e| ExportError::Docx(format!("{:?}", e)))?;
    let header_tables = header_row_tables.iter().filter(|flag| **flag).count();
    inject_table_header_row_flags(&mut buf, header_tables)?;
    inject_footnotes(&mut buf, doc_json)?;
    Ok(buf)
}

/// docx-rs 0.4 cannot emit `w:tblHeader`. Tables whose first row is made of
/// `table_header` cells were counted during export (`header_tables`, in
/// document order); inject `<w:tblHeader/>` into the first `<w:tr>`'s
/// `<w:trPr>` of each such table by rewriting the packed `word/document.xml`.
fn inject_table_header_row_flags(
    buf: &mut Vec<u8>,
    header_tables: usize,
) -> Result<(), ExportError> {
    if header_tables == 0 {
        return Ok(());
    }
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(&*buf))
        .map_err(|e| ExportError::Docx(e.to_string()))?;
    let mut document = String::new();
    archive
        .by_name("word/document.xml")
        .map_err(|e| ExportError::Docx(e.to_string()))?
        .read_to_string(&mut document)
        .map_err(|e| ExportError::Docx(e.to_string()))?;
    let mut rebuilt = String::with_capacity(document.len());
    let mut rest = document.as_str();
    let mut remaining_headers = header_tables;
    while let Some(open) = rest.find("<w:tbl>") {
        // Scan to this table's true close tag, accounting for nested tables.
        // Only exact "<w:tbl>" opens and "</w:tbl>" closes count; tblPr /
        // tblGrid / tblHeader elements share the prefix but not the tag.
        let mut depth = 0usize;
        let mut cursor = open;
        let mut table_end = None;
        loop {
            let next_open = rest[cursor..].find("<w:tbl>").map(|at| cursor + at);
            let next_close = rest[cursor..].find("</w:tbl>").map(|at| cursor + at);
            match (next_open, next_close) {
                (Some(o), Some(c)) if o < c => {
                    depth += 1;
                    cursor = o + "<w:tbl>".len();
                }
                (_, Some(c)) => {
                    cursor = c + "</w:tbl>".len();
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        table_end = Some(c);
                        break;
                    }
                }
                _ => break,
            }
        }
        let Some(table_end) = table_end else {
            break;
        };
        let table_xml = &rest[..table_end + "</w:tbl>".len()];
        rest = &rest[table_end + "</w:tbl>".len()..];
        if remaining_headers > 0 {
            let first_row_end = table_xml.find("</w:tr>");
            let is_candidate = table_xml
                .find("<w:tr>")
                .is_some_and(|row_open| first_row_end.is_some_and(|end| row_open < end));
            if is_candidate {
                if let Some(row_open) = table_xml.find("<w:tr>") {
                    let row_end = first_row_end.unwrap_or(0);
                    let row_xml = &table_xml[row_open..row_end];
                    let mut patched = String::with_capacity(row_xml.len() + 32);
                    if let Some(pr) = row_xml.find("<w:trPr>") {
                        patched.push_str(&row_xml[..pr + "<w:trPr>".len()]);
                        patched.push_str("<w:tblHeader/>");
                        patched.push_str(&row_xml[pr + "<w:trPr>".len()..]);
                    } else if let Some(pr) = row_xml.find("<w:trPr />") {
                        patched.push_str(&row_xml[..pr]);
                        patched.push_str("<w:trPr><w:tblHeader/></w:trPr>");
                        patched.push_str(&row_xml[pr + "<w:trPr />".len()..]);
                    } else {
                        patched.push_str("<w:tr><w:trPr><w:tblHeader/></w:trPr>");
                        patched.push_str(&row_xml["<w:tr>".len()..]);
                    }
                    rebuilt.push_str(&table_xml[..row_open]);
                    rebuilt.push_str(&patched);
                    rebuilt.push_str(&table_xml[row_end..]);
                    remaining_headers -= 1;
                    continue;
                }
            }
        }
        rebuilt.push_str(table_xml);
    }
    rebuilt.push_str(rest);

    // Rewrite the package with the patched document.xml.
    let cursor = std::io::Cursor::new(Vec::new());
    let mut writer = zip::ZipWriter::new(cursor);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| ExportError::Docx(e.to_string()))?;
        let name = entry.name().to_string();
        // Skip explicit directory entries; OOXML readers do not need them and
        // duplicating them changes the media-entry layout between runs.
        if entry.is_dir() || name.ends_with('/') {
            continue;
        }
        let mut data = Vec::new();
        entry
            .read_to_end(&mut data)
            .map_err(|e| ExportError::Docx(e.to_string()))?;
        drop(entry);
        if name == "word/document.xml" {
            data = rebuilt.clone().into_bytes();
        }
        writer
            .start_file(name, options)
            .map_err(|e| ExportError::Docx(e.to_string()))?;
        writer
            .write_all(&data)
            .map_err(|e| ExportError::Docx(e.to_string()))?;
    }
    let cursor = writer
        .finish()
        .map_err(|e| ExportError::Docx(e.to_string()))?;
    *buf = cursor.into_inner();
    Ok(())
}

/// Rewrite the packed DOCX with native footnote parts when the document has
/// `footnote_ref` nodes. Marker runs `[[FN:id|label]]` emitted by
/// `add_runs_to_paragraph` become `<w:footnoteReference w:id>` runs; a real
/// `word/footnotes.xml` part replaces the docx-rs default with the texts
/// from `doc_json.footnotes`.
fn inject_footnotes(buf: &mut Vec<u8>, doc_json: &Value) -> Result<(), ExportError> {
    let notes = doc_json
        .get("footnotes")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default();
    if notes.is_empty() {
        return Ok(());
    }
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(&*buf))
        .map_err(|e| ExportError::Docx(e.to_string()))?;
    let mut document = String::new();
    archive
        .by_name("word/document.xml")
        .map_err(|e| ExportError::Docx(e.to_string()))?
        .read_to_string(&mut document)
        .map_err(|e| ExportError::Docx(e.to_string()))?;
    if !document.contains("[[FN:") {
        return Ok(());
    }

    // Word footnote ids: 0=separator, 1=continuationSeparator, notes at 2+.
    let id_by_note: HashMap<&str, usize> = notes
        .iter()
        .enumerate()
        .filter_map(|(index, note)| {
            note.get("id")
                .and_then(Value::as_str)
                .map(|id| (id, index + 2))
        })
        .collect();
    if id_by_note.is_empty() {
        return Ok(());
    }

    // Replace marker runs with footnote references. Marker runs are emitted as
    // <w:r><w:rPr>...</w:rPr><w:t xml:space="preserve">[[FN:id|label]]</w:t></w:r>.
    let mut rebuilt = String::with_capacity(document.len());
    let mut rest = document.as_str();
    while let Some(marker_start) = rest.find("[[FN:") {
        let prefix_end = marker_start + "[[FN:".len();
        let Some(close) = rest[prefix_end..].find("]]") else {
            break;
        };
        let body = &rest[prefix_end..prefix_end + close];
        let Some((id, _label)) = body.split_once('|') else {
            break;
        };
        rebuilt.push_str(&rest[..marker_start]);
        let after_marker = &rest[prefix_end + close + 2..];
        match id_by_note.get(id) {
            Some(word_id) => {
                // Everything up to the marker text was just emitted; rewind
                // to the opening tag of the run that carries the marker and
                // replace the whole run with a footnoteReference run.
                let run_open = rest[..marker_start].rfind("<w:r>").unwrap_or(0);
                rebuilt.truncate(rebuilt.len() - (marker_start - run_open));
                let run_close = after_marker.find("</w:r>").unwrap_or(0);
                rebuilt.push_str(&format!(
                    "<w:r><w:rPr><w:vertAlign w:val=\"superscript\"/></w:rPr><w:footnoteReference w:id=\"{word_id}\"/></w:r>"
                ));
                rest = &after_marker[run_close + "</w:r>".len()..];
            }
            None => {
                // Unknown note id: drop just the marker text, keep the run.
                rest = after_marker;
            }
        }
    }
    rebuilt.push_str(rest);

    // Build word/footnotes.xml.
    let escape = |value: &str| -> String {
        value
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
    };
    let mut footnotes_xml = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<w:footnotes xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\">",
    );
    footnotes_xml.push_str("<w:footnote w:type=\"separator\" w:id=\"0\"><w:p><w:pPr><w:spacing w:after=\"0\" w:line=\"240\" w:lineRule=\"auto\"/></w:pPr><w:r><w:separator/></w:r></w:p></w:footnote>");
    footnotes_xml.push_str("<w:footnote w:type=\"continuationSeparator\" w:id=\"1\"><w:p><w:pPr><w:spacing w:after=\"0\" w:line=\"240\" w:lineRule=\"auto\"/></w:pPr><w:r><w:continuationSeparator/></w:r></w:p></w:footnote>");
    for (index, note) in notes.iter().enumerate() {
        let text = note.get("text").and_then(Value::as_str).unwrap_or_default();
        let label = note
            .get("label")
            .and_then(Value::as_u64)
            .unwrap_or(index as u64 + 1);
        footnotes_xml.push_str(&format!(
            "<w:footnote w:id=\"{}\"><w:p><w:pPr><w:pStyle w:val=\"FootnoteText\"/></w:pPr><w:r><w:rPr><w:vertAlign w:val=\"superscript\"/></w:rPr><w:t xml:space=\"preserve\">{} </w:t></w:r><w:r><w:t xml:space=\"preserve\">{}</w:t></w:r></w:p></w:footnote>",
            index + 2,
            label,
            escape(text),
        ));
    }
    footnotes_xml.push_str("</w:footnotes>");

    // Patch content types and document relationships, then rewrite the package.
    let mut content_types = String::new();
    archive
        .by_name("[Content_Types].xml")
        .map_err(|e| ExportError::Docx(e.to_string()))?
        .read_to_string(&mut content_types)
        .map_err(|e| ExportError::Docx(e.to_string()))?;
    let mut document_rels = String::new();
    archive
        .by_name("word/_rels/document.xml.rels")
        .map_err(|e| ExportError::Docx(e.to_string()))?
        .read_to_string(&mut document_rels)
        .map_err(|e| ExportError::Docx(e.to_string()))?;

    let footnotes_type =
        "application/vnd.openxmlformats-officedocument.wordprocessingml.footnotes+xml";
    let content_types = if content_types.contains("footnotes+xml") {
        content_types
    } else {
        content_types.replacen(
            "</Types>",
            &format!(
                "<Override PartName=\"/word/footnotes.xml\" ContentType=\"{footnotes_type}\"/></Types>"
            ),
            1,
        )
    };
    let next_id = document_rels
        .match_indices("Id=\"rId")
        .filter_map(|(offset, _)| {
            let tail = &document_rels[offset + 8..];
            let end = tail.find('"')?;
            tail[..end].parse::<u32>().ok()
        })
        .max()
        .unwrap_or(0)
        + 1;
    let document_rels = if document_rels.contains("footnotes.xml") {
        document_rels
    } else {
        document_rels.replacen(
            "</Relationships>",
            &format!(
                "<Relationship Id=\"rId{next_id}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/footnotes\" Target=\"footnotes.xml\"/></Relationships>"
            ),
            1,
        )
    };

    let cursor = std::io::Cursor::new(Vec::new());
    let mut writer = zip::ZipWriter::new(cursor);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| ExportError::Docx(e.to_string()))?;
        let name = entry.name().to_string();
        if entry.is_dir() || name.ends_with('/') {
            continue;
        }
        let mut data = Vec::new();
        entry
            .read_to_end(&mut data)
            .map_err(|e| ExportError::Docx(e.to_string()))?;
        drop(entry);
        // docx-rs already ships a default word/footnotes.xml; replace it.
        let data = match name.as_str() {
            "word/document.xml" => rebuilt.clone().into_bytes(),
            "[Content_Types].xml" => content_types.clone().into_bytes(),
            "word/_rels/document.xml.rels" => document_rels.clone().into_bytes(),
            "word/footnotes.xml" => footnotes_xml.clone().into_bytes(),
            _ => data,
        };
        writer
            .start_file(name, options)
            .map_err(|e| ExportError::Docx(e.to_string()))?;
        writer
            .write_all(&data)
            .map_err(|e| ExportError::Docx(e.to_string()))?;
    }
    let cursor = writer
        .finish()
        .map_err(|e| ExportError::Docx(e.to_string()))?;
    *buf = cursor.into_inner();
    Ok(())
}

pub fn import_docx_to_doc_with_report(path: &Path) -> Result<DocxImportResult, ExportError> {
    let file_size = std::fs::metadata(path)?.len();
    if file_size > MAX_DOCX_FILE_BYTES {
        return Err(ExportError::Docx(format!(
            "DOCX file exceeds the {MAX_DOCX_FILE_BYTES}-byte safety limit"
        )));
    }
    let file = std::fs::File::open(path)?;
    let mut archive = zip::ZipArchive::new(file)?;
    if archive.len() > MAX_DOCX_ARCHIVE_ENTRIES {
        return Err(ExportError::Docx(format!(
            "DOCX archive has too many entries (maximum {MAX_DOCX_ARCHIVE_ENTRIES})"
        )));
    }
    let document = read_docx_xml(&mut archive, "word/document.xml")?;
    let imported_comments = read_docx_comments(&mut archive)?;
    // Native footnotes: word/footnotes.xml word-id -> { id, label, text }.
    let imported_footnotes = read_docx_footnotes(&mut archive);

    let mut image_data: HashMap<String, String> = HashMap::new();
    let mut hyperlink_data: HashMap<String, String> = HashMap::new();
    let mut relationship_targets: HashMap<String, (String, String)> = HashMap::new();
    let mut warnings = Vec::new();
    let relationships_xml = match archive.by_name("word/_rels/document.xml.rels") {
        Ok(mut relationships) => {
            if relationships.size() > MAX_DOCX_XML_BYTES {
                return Err(ExportError::Docx(format!(
                    "DOCX relationships entry exceeds the {MAX_DOCX_XML_BYTES}-byte safety limit"
                )));
            }
            let mut bytes = Vec::with_capacity(relationships.size() as usize);
            relationships.read_to_end(&mut bytes)?;
            Some(String::from_utf8(bytes).map_err(|error| ExportError::Docx(error.to_string()))?)
        }
        Err(_) => None,
    };
    let mut total_media_bytes = 0_u64;
    if let Some(xml) = relationships_xml {
        let mut rel_reader = Reader::from_str(&xml);
        let mut rel_buffer = Vec::new();
        loop {
            match rel_reader.read_event_into(&mut rel_buffer) {
                Ok(Event::Empty(event)) | Ok(Event::Start(event))
                    if event.name().as_ref() == b"Relationship" =>
                {
                    let mut id = None;
                    let mut target = None;
                    let mut rel_type = None;
                    for attribute in event.attributes().flatten() {
                        match attribute.key.as_ref() {
                            b"Id" => {
                                id = Some(String::from_utf8_lossy(&attribute.value).into_owned())
                            }
                            b"Target" => {
                                target =
                                    Some(String::from_utf8_lossy(&attribute.value).into_owned())
                            }
                            b"Type" => {
                                rel_type =
                                    Some(String::from_utf8_lossy(&attribute.value).into_owned())
                            }
                            _ => {}
                        }
                    }
                    if let (Some(id), Some(target)) = (id, target) {
                        relationship_targets.insert(
                            id.clone(),
                            (target.clone(), rel_type.clone().unwrap_or_default()),
                        );
                        if rel_type.as_deref().is_some_and(|t| t.contains("hyperlink")) {
                            hyperlink_data.insert(id, target);
                            continue;
                        }
                        let Some(package_path) = docx_part_path(&target) else {
                            warnings.push(format!(
                                "Skipped DOCX relationship target '{target}': unsafe package path"
                            ));
                            continue;
                        };
                        if let Ok(mut media) = archive.by_name(&package_path) {
                            let media_size = media.size();
                            if media_size > MAX_DOCX_MEDIA_BYTES
                                || total_media_bytes.saturating_add(media_size)
                                    > MAX_DOCX_TOTAL_MEDIA_BYTES
                            {
                                warnings.push(format!(
                                    "Skipped DOCX media entry {package_path}: size exceeds import limits"
                                ));
                                continue;
                            }
                            let mut bytes = Vec::new();
                            media.read_to_end(&mut bytes)?;
                            total_media_bytes =
                                total_media_bytes.saturating_add(bytes.len() as u64);
                            let mime = match target
                                .rsplit('.')
                                .next()
                                .unwrap_or_default()
                                .to_ascii_lowercase()
                                .as_str()
                            {
                                "jpg" | "jpeg" => "image/jpeg",
                                "gif" => "image/gif",
                                "webp" => "image/webp",
                                _ => "image/png",
                            };
                            image_data.insert(
                                id,
                                format!("data:{mime};base64,{}", encode_base64(&bytes)),
                            );
                        }
                    }
                }
                Ok(Event::Eof) => break,
                Ok(_) => {}
                Err(error) => return Err(ExportError::Docx(error.to_string())),
            }
            rel_buffer.clear();
        }
    }

    let imported_page_setup = parse_docx_page_setup(
        &document,
        &relationship_targets,
        &mut archive,
        &mut warnings,
    )?;

    let mut num_formats: HashMap<u32, String> = HashMap::new();
    let mut num_starts: HashMap<u32, u64> = HashMap::new();
    if let Ok(mut numbering) = archive.by_name("word/numbering.xml") {
        let mut numbering_xml = String::new();
        numbering.read_to_string(&mut numbering_xml)?;
        let mut abs_formats: HashMap<String, String> = HashMap::new();
        for part in numbering_xml.split("<w:abstractNum").skip(1) {
            let abs_id = part.split('"').nth(1).unwrap_or_default();
            if let Some(fmt_start) = part.find("<w:numFmt") {
                let slice = &part[fmt_start..];
                if let Some(val_start) = slice.find("w:val=\"") {
                    let rest = &slice[val_start + 7..];
                    if let Some(end) = rest.find('"') {
                        abs_formats.insert(abs_id.to_string(), rest[..end].to_string());
                    }
                }
            }
        }
        for part in numbering_xml.split("<w:num").skip(1) {
            let num_id = part.split('"').nth(1).and_then(|s| s.parse().ok());
            if let Some(abs_pos) = part.find("w:abstractNumId") {
                let slice = &part[abs_pos..];
                if let Some(val_start) = slice.find("w:val=\"") {
                    let rest = &slice[val_start + 7..];
                    if let Some(end) = rest.find('"') {
                        let abs_id = rest[..end].to_string();
                        if let (Some(num_id), Some(fmt)) = (num_id, abs_formats.get(&abs_id)) {
                            num_formats.insert(num_id, fmt.clone());
                        }
                    }
                }
            }
            // lvlOverride startOverride overrides the level-0 start value.
            if let Some(num_id) = num_id {
                if let Some(ovr) = part.find("<w:lvlOverride") {
                    let slice = &part[ovr..];
                    if let Some(start_pos) = slice.find("<w:startOverride") {
                        let tail = &slice[start_pos..];
                        if let Some(val_start) = tail.find("w:val=\"") {
                            let rest = &tail[val_start + 7..];
                            if let Some(end) = rest.find('"') {
                                if let Ok(start) = rest[..end].parse::<u64>() {
                                    num_starts.insert(num_id, start.clamp(1, 999));
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    let mut reader = Reader::from_str(&document);
    reader.config_mut().trim_text(true);
    let mut paragraphs = Vec::new();
    // Table state is a stack so nested tables (w:tbl inside w:tc) import
    // structurally instead of being flattened into the paragraph stream.
    let mut table_stack: Vec<Vec<Value>> = Vec::new();
    let mut row_cells: Option<Vec<Value>> = None;
    let mut cell_content: Option<Vec<Value>> = None;
    // Cell contents of enclosing tables while a nested table is being parsed.
    let mut cell_content_stack: Vec<Vec<Value>> = Vec::new();
    // Partial rows of enclosing tables while a nested table is being parsed.
    let mut row_cells_stack: Vec<Vec<Value>> = Vec::new();
    let mut current_content: Vec<Value> = Vec::new();
    let mut current_text = String::new();
    let mut run_state = RunImportState::new();
    let mut paragraph_style: Option<String> = None;
    let mut paragraph_num_id: Option<u32> = None;
    // Paragraph layout attributes (w:jc / w:ind / w:spacing) collected while
    // the paragraph is open and merged into the node at </w:p>.
    let mut paragraph_align: Option<String> = None;
    let mut paragraph_indent: Option<f64> = None;
    let mut paragraph_line_height: Option<f64> = None;
    let mut paragraph_spacing_before: Option<f64> = None;
    let mut paragraph_spacing_after: Option<f64> = None;
    let mut in_text = false;
    let mut pending_image: Option<Value> = None;
    let mut active_hyperlink: Option<String> = None;
    let mut active_tracked_change: Option<TrackedChangeContext> = None;
    let mut document_position = 0usize;
    let mut paragraph_start = 0usize;
    let mut paragraph_text_units = 0usize;
    let mut pending_page_break = false;
    let mut comment_ranges: HashMap<usize, (usize, usize)> = HashMap::new();
    let mut open_list: Option<OpenList> = None;
    let mut open_bookmarks: Vec<(usize, String)> = Vec::new();
    let mut closed_bookmarks: HashMap<usize, String> = HashMap::new();
    let mut in_field_instruction = false;
    let mut field_instruction_text = String::new();
    let mut field_active = false;
    let mut field_seen_instruction = false;
    let mut field_in_result = false;
    // Table import state: current cell's grid span / vertical merge and the
    // row's tblHeader flag.
    let mut cell_grid_span: usize = 1;
    let mut cell_vmerge_restart = false;
    let mut cell_vmerge_continue = false;
    let mut row_is_header = false;
    // tblGrid parsing: column widths (twips) per open table (stack, so
    // nested tables keep independent grids).
    let mut in_table_grid = false;
    let mut table_grid_widths_stack: Vec<Vec<usize>> = Vec::new();
    let mut buffer = Vec::new();

    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(event)) => match event.name().as_ref() {
                b"w:tbl" => {
                    table_stack.push(Vec::new());
                    table_grid_widths_stack.push(Vec::new());
                }
                b"w:tblGrid" => in_table_grid = true,
                b"w:tr" => {
                    if let Some(existing) = row_cells.take() {
                        row_cells_stack.push(existing);
                    }
                    row_cells = Some(Vec::new());
                }
                b"w:tc" => {
                    // Preserve the enclosing cell's content while a nested
                    // table is open inside it.
                    if let Some(existing) = cell_content.take() {
                        cell_content_stack.push(existing);
                    }
                    cell_content = Some(Vec::new());
                }
                b"w:p" => {
                    paragraph_start = document_position;
                    paragraph_text_units = 0;
                    current_content = Vec::new();
                    paragraph_style = None;
                    paragraph_num_id = None;
                    paragraph_align = None;
                    paragraph_indent = None;
                    paragraph_line_height = None;
                    paragraph_spacing_before = None;
                    paragraph_spacing_after = None;
                }
                b"w:commentRangeStart" => {
                    if let Some(id) = docx_attr(&event, b"id").and_then(|value| value.parse().ok())
                    {
                        let position = paragraph_start
                            .saturating_add(1)
                            .saturating_add(paragraph_text_units);
                        comment_ranges.insert(id, (position, position));
                    }
                }
                b"w:bookmarkStart" => {
                    if let (Some(id), Some(name)) = (
                        docx_attr(&event, b"id").and_then(|value| value.parse::<usize>().ok()),
                        docx_attr(&event, b"name"),
                    ) {
                        if !name.trim().is_empty() {
                            open_bookmarks.push((id, name));
                        }
                    }
                }
                b"w:ins" | b"w:del" => {
                    let kind = if event.name().as_ref() == b"w:ins" {
                        "insert"
                    } else {
                        "delete"
                    };
                    let mut author = String::from("Unknown");
                    let mut change_id = String::new();
                    let mut created_at = String::new();
                    for attribute in event.attributes().flatten() {
                        match attribute.key.as_ref() {
                            b"w:author" => {
                                author = String::from_utf8_lossy(&attribute.value).into_owned()
                            }
                            b"w:id" => {
                                change_id = String::from_utf8_lossy(&attribute.value).into_owned()
                            }
                            b"w:date" => {
                                created_at = String::from_utf8_lossy(&attribute.value).into_owned()
                            }
                            _ => {}
                        }
                    }
                    active_tracked_change = Some(TrackedChangeContext {
                        kind,
                        author,
                        change_id,
                        created_at,
                    });
                }
                b"w:hyperlink" => {
                    for attribute in event.attributes().flatten() {
                        match attribute.key.as_ref() {
                            b"r:id" => {
                                let id = String::from_utf8_lossy(&attribute.value).to_string();
                                active_hyperlink = hyperlink_data.get(&id).cloned();
                            }
                            // Internal anchors link back to bookmarks.
                            b"w:anchor" => {
                                let anchor = String::from_utf8_lossy(&attribute.value).to_string();
                                if !anchor.trim().is_empty() {
                                    active_hyperlink = Some(format!("internal:{anchor}"));
                                }
                            }
                            _ => {}
                        }
                    }
                }
                b"w:instrText" => {
                    // Body PAGE / NUMPAGES field instructions become native
                    // field nodes; the cached result between separate and end
                    // is dropped because the editor recomputes it.
                    current_text.clear();
                    in_field_instruction = true;
                    field_instruction_text.clear();
                }
                b"w:pStyle" => {
                    for attribute in event.attributes().flatten() {
                        if attribute.key.as_ref() == b"w:val" {
                            paragraph_style =
                                Some(String::from_utf8_lossy(&attribute.value).to_string());
                        }
                    }
                }
                b"w:b" => run_state.bold = true,
                b"w:i" => run_state.italic = true,
                b"w:u" => run_state.underline = true,
                b"w:strike" => run_state.strike = true,
                b"w:sz" => {
                    for attribute in event.attributes().flatten() {
                        if attribute.key.as_ref() == b"w:val" {
                            if let Ok(half_pts) =
                                String::from_utf8_lossy(&attribute.value).parse::<f64>()
                            {
                                run_state.font_size = Some(format!("{}", half_pts / 2.0));
                            }
                        }
                    }
                }
                b"w:color" => {
                    for attribute in event.attributes().flatten() {
                        if attribute.key.as_ref() == b"w:val" {
                            let hex = String::from_utf8_lossy(&attribute.value).to_string();
                            run_state.color = Some(format!("#{}", hex.trim_start_matches('#')));
                        }
                    }
                }
                b"w:highlight" => {
                    for attribute in event.attributes().flatten() {
                        if attribute.key.as_ref() == b"w:val" {
                            let name = String::from_utf8_lossy(&attribute.value).to_string();
                            run_state.highlight = Some(word_highlight_to_css(&name));
                        }
                    }
                }
                b"w:vertAlign" => {
                    for attribute in event.attributes().flatten() {
                        if attribute.key.as_ref() == b"w:val" {
                            match String::from_utf8_lossy(&attribute.value).as_ref() {
                                "superscript" => run_state.superscript = true,
                                "subscript" => run_state.subscript = true,
                                _ => {}
                            }
                        }
                    }
                }
                b"w:rFonts" => {
                    for attribute in event.attributes().flatten() {
                        if attribute.key.as_ref() == b"w:ascii" {
                            run_state.font_family =
                                Some(String::from_utf8_lossy(&attribute.value).to_string());
                        }
                    }
                }
                b"w:numId" => {
                    for attribute in event.attributes().flatten() {
                        if attribute.key.as_ref() == b"w:val" {
                            paragraph_num_id =
                                String::from_utf8_lossy(&attribute.value).parse().ok();
                        }
                    }
                }
                b"w:t" | b"w:delText" => {
                    current_text.clear();
                    in_text = true;
                }
                _ => record_unsupported_docx_construct(&mut warnings, event.name().as_ref()),
            },
            Ok(Event::Text(event)) if in_text || in_field_instruction => {
                let value = event
                    .unescape()
                    .map_err(|error| ExportError::Docx(error.to_string()))?;
                if in_field_instruction {
                    field_instruction_text.push_str(&value);
                    field_seen_instruction = true;
                    in_field_instruction = false;
                } else {
                    current_text.push_str(&value);
                }
            }
            Ok(Event::Empty(event)) => match event.name().as_ref() {
                b"w:commentRangeStart" => {
                    if let Some(id) = docx_attr(&event, b"id").and_then(|value| value.parse().ok())
                    {
                        let position = paragraph_start
                            .saturating_add(1)
                            .saturating_add(paragraph_text_units);
                        comment_ranges.insert(id, (position, position));
                    }
                }
                b"w:commentRangeEnd" => {
                    if let Some(id) = docx_attr(&event, b"id").and_then(|value| value.parse().ok())
                    {
                        let position = paragraph_start
                            .saturating_add(1)
                            .saturating_add(paragraph_text_units);
                        if let Some(range) = comment_ranges.get_mut(&id) {
                            range.1 = position;
                        }
                    }
                }
                b"w:commentReference" => {}
                b"w:bookmarkStart" => {
                    if let (Some(id), Some(name)) = (
                        docx_attr(&event, b"id").and_then(|value| value.parse::<usize>().ok()),
                        docx_attr(&event, b"name"),
                    ) {
                        if !name.trim().is_empty() {
                            open_bookmarks.push((id, name));
                        }
                    }
                }
                b"w:tblHeader" => {
                    row_is_header = true;
                }
                b"w:gridCol" if in_table_grid => {
                    // Column width in twips -> px at 96dpi (export inverse:
                    // px * 15). Keeps user-resized columns on round-trip.
                    if let Some(twips) =
                        docx_attr(&event, b"w").and_then(|value| value.parse::<usize>().ok())
                    {
                        if let Some(widths) = table_grid_widths_stack.last_mut() {
                            widths.push((twips / 15).clamp(1, 2112));
                        }
                    }
                }
                b"w:gridSpan" => {
                    cell_grid_span = docx_attr(&event, b"val")
                        .and_then(|value| value.parse::<usize>().ok())
                        .unwrap_or(1)
                        .clamp(1, 100);
                }
                b"w:vMerge" => {
                    // vMerge with no val, or val="continue", is a covered
                    // cell of a vertical merge; val="restart" starts one.
                    let value = docx_attr(&event, b"val").unwrap_or_default();
                    if value.eq_ignore_ascii_case("restart") {
                        cell_vmerge_restart = true;
                    } else {
                        cell_vmerge_continue = true;
                    }
                }
                b"w:jc" => {
                    paragraph_align = docx_attr(&event, b"val").map(|value| match value.as_str() {
                        "center" => "center".to_string(),
                        "right" => "right".to_string(),
                        "both" | "justify" => "justify".to_string(),
                        _ => "left".to_string(),
                    });
                }
                b"w:ind" => {
                    if let Some(left) = docx_attr(&event, b"left")
                        .and_then(|value| value.parse::<f64>().ok())
                        .filter(|value| *value > 0.0)
                    {
                        paragraph_indent = Some((left / 720.0 * 1000.0).round() / 1000.0);
                    }
                }
                b"w:spacing" => {
                    if let Some(line) = docx_attr(&event, b"line")
                        .and_then(|value| value.parse::<f64>().ok())
                        .filter(|value| *value >= 0.0)
                    {
                        paragraph_line_height = Some((line / 240.0 * 1000.0).round() / 1000.0);
                    }
                    if let Some(before) = docx_attr(&event, b"before")
                        .and_then(|value| value.parse::<f64>().ok())
                        .filter(|value| *value >= 0.0)
                    {
                        paragraph_spacing_before = Some((before / 20.0 * 10.0).round() / 10.0);
                    }
                    if let Some(after) = docx_attr(&event, b"after")
                        .and_then(|value| value.parse::<f64>().ok())
                        .filter(|value| *value >= 0.0)
                    {
                        paragraph_spacing_after = Some((after / 20.0 * 10.0).round() / 10.0);
                    }
                }
                b"w:fldChar" => match docx_attr(&event, b"fldCharType").as_deref() {
                    Some("begin") => {
                        field_active = true;
                        field_seen_instruction = false;
                    }
                    Some("separate") => {
                        field_in_result = true;
                    }
                    Some("end") => {
                        flush_field_node(
                            &mut current_content,
                            &mut paragraph_text_units,
                            field_active,
                            field_seen_instruction,
                            &field_instruction_text,
                            &mut warnings,
                        );
                        field_active = false;
                        field_seen_instruction = false;
                        field_in_result = false;
                        field_instruction_text.clear();
                    }
                    _ => {}
                },
                b"w:footnoteReference" => {
                    // Map the Word footnote id back to a footnote_ref node.
                    if let Some(note) = docx_attr(&event, b"id")
                        .and_then(|id| id.parse::<u32>().ok())
                        .and_then(|id| imported_footnotes.get(&id).cloned())
                    {
                        current_content.push(json!({
                            "type": "footnote_ref",
                            "attrs": { "id": note["id"], "label": note["label"] }
                        }));
                    }
                }
                b"w:br" => {
                    if docx_attr(&event, b"type").as_deref() == Some("page") {
                        pending_page_break = true;
                    } else {
                        record_unsupported_docx_construct(&mut warnings, event.name().as_ref());
                    }
                }
                b"w:b" => run_state.bold = true,
                b"w:i" => run_state.italic = true,
                b"w:u" => run_state.underline = true,
                b"w:strike" => run_state.strike = true,
                b"w:sz" => {
                    for attribute in event.attributes().flatten() {
                        if attribute.key.as_ref() == b"w:val" {
                            if let Ok(half_pts) =
                                String::from_utf8_lossy(&attribute.value).parse::<f64>()
                            {
                                run_state.font_size = Some(format!("{}", half_pts / 2.0));
                            }
                        }
                    }
                }
                b"w:color" => {
                    for attribute in event.attributes().flatten() {
                        if attribute.key.as_ref() == b"w:val" {
                            let hex = String::from_utf8_lossy(&attribute.value).to_string();
                            run_state.color = Some(format!("#{}", hex.trim_start_matches('#')));
                        }
                    }
                }
                b"w:highlight" => {
                    for attribute in event.attributes().flatten() {
                        if attribute.key.as_ref() == b"w:val" {
                            let name = String::from_utf8_lossy(&attribute.value).to_string();
                            run_state.highlight = Some(word_highlight_to_css(&name));
                        }
                    }
                }
                b"w:vertAlign" => {
                    for attribute in event.attributes().flatten() {
                        if attribute.key.as_ref() == b"w:val" {
                            match String::from_utf8_lossy(&attribute.value).as_ref() {
                                "superscript" => run_state.superscript = true,
                                "subscript" => run_state.subscript = true,
                                _ => {}
                            }
                        }
                    }
                }
                b"w:rFonts" => {
                    for attribute in event.attributes().flatten() {
                        if attribute.key.as_ref() == b"w:ascii" {
                            run_state.font_family =
                                Some(String::from_utf8_lossy(&attribute.value).to_string());
                        }
                    }
                }
                b"w:numId" => {
                    for attribute in event.attributes().flatten() {
                        if attribute.key.as_ref() == b"w:val" {
                            paragraph_num_id =
                                String::from_utf8_lossy(&attribute.value).parse().ok();
                        }
                    }
                }
                b"w:pStyle" => {
                    for attribute in event.attributes().flatten() {
                        if attribute.key.as_ref() == b"w:val" {
                            paragraph_style =
                                Some(String::from_utf8_lossy(&attribute.value).to_string());
                        }
                    }
                }
                b"a:blip" => {
                    for attribute in event.attributes().flatten() {
                        if attribute.key.as_ref() == b"r:embed" {
                            if let Some(src) = image_data
                                .get(&String::from_utf8_lossy(&attribute.value).to_string())
                            {
                                pending_image = Some(json!({
                                    "type": "image",
                                    "attrs": { "src": src, "alt": "" }
                                }));
                            }
                        }
                    }
                }
                _ => record_unsupported_docx_construct(&mut warnings, event.name().as_ref()),
            },
            Ok(Event::End(event)) => match event.name().as_ref() {
                b"w:t" | b"w:delText" => in_text = false,
                b"w:instrText" => in_field_instruction = false,
                b"w:r" => {
                    paragraph_text_units =
                        paragraph_text_units.saturating_add(utf16_len(&current_text));
                    // Cached field results are dropped; the editor recomputes.
                    if !field_in_result && !current_text.is_empty() {
                        let mut marks = run_state
                            .to_marks(active_hyperlink.as_deref(), active_tracked_change.as_ref());
                        // Open bookmark names become bookmark marks over the
                        // remaining runs until their bookmarkEnd closes.
                        for (id, name) in &open_bookmarks {
                            if !closed_bookmarks.contains_key(id) {
                                marks.push(json!({
                                    "type": "bookmark",
                                    "attrs": { "id": null, "name": name }
                                }));
                            }
                        }
                        let mut run = json!({ "type": "text", "text": current_text });
                        if !marks.is_empty() {
                            run["marks"] = Value::Array(marks);
                        }
                        current_content.push(run);
                    }
                    current_text.clear();
                    run_state.clear();
                }
                b"w:bookmarkEnd" => {
                    // End events carry no attributes in quick-xml; close the
                    // oldest open bookmark (Word emits ends in start order).
                    if let Some((id, _)) = open_bookmarks.first().cloned() {
                        open_bookmarks.remove(0);
                        closed_bookmarks.insert(id, String::new());
                    }
                }
                b"w:hyperlink" => {
                    active_hyperlink = None;
                }
                b"w:ins" | b"w:del" => {
                    active_tracked_change = None;
                }
                b"w:p" => {
                    document_position = paragraph_start
                        .saturating_add(paragraph_text_units)
                        .saturating_add(2);
                    let is_heading = paragraph_style
                        .as_deref()
                        .is_some_and(|style| style.starts_with("Heading"));
                    let mut node = if is_heading {
                        let level = paragraph_style
                            .as_deref()
                            .and_then(|style| {
                                style.trim_start_matches("Heading").parse::<u64>().ok()
                            })
                            .unwrap_or(1);
                        json!({ "type": "heading", "attrs": { "level": level.clamp(1, 6) }, "content": current_content })
                    } else {
                        let style_name = paragraph_style
                            .as_deref()
                            .filter(|style| !style.is_empty() && *style != "Normal");
                        json!({ "type": "paragraph", "attrs": { "styleName": style_name }, "content": current_content })
                    };
                    // Paragraph layout attributes imported from w:jc/w:ind/
                    // w:spacing so alignment and spacing round-trip.
                    if let Some(attrs) = node.get_mut("attrs").and_then(Value::as_object_mut) {
                        if let Some(align) = paragraph_align.take() {
                            attrs.insert("align".to_string(), json!(align));
                        }
                        if let Some(indent) = paragraph_indent.take() {
                            attrs.insert("indent".to_string(), json!(indent));
                        }
                        if let Some(line_height) = paragraph_line_height.take() {
                            attrs.insert("lineHeight".to_string(), json!(line_height));
                        }
                        if let Some(before) = paragraph_spacing_before.take() {
                            attrs.insert("spacingBefore".to_string(), json!(before));
                        }
                        if let Some(after) = paragraph_spacing_after.take() {
                            attrs.insert("spacingAfter".to_string(), json!(after));
                        }
                    }
                    if let Some(image) = pending_image.take() {
                        if let Some(content) = node.get_mut("content").and_then(Value::as_array_mut)
                        {
                            content.push(image);
                        }
                    }
                    let node_has_content = node
                        .get("content")
                        .and_then(Value::as_array)
                        .is_some_and(|content| !content.is_empty());
                    if node_has_content || !pending_page_break {
                        if let Some(content) = cell_content.as_mut() {
                            // Numbered/bulleted paragraphs inside table cells
                            // keep their list grouping instead of degrading to
                            // plain paragraphs.
                            if let Some(num_id) = paragraph_num_id {
                                let list_type = list_type_for_num_id(num_id, &num_formats);
                                let list_order = num_starts.get(&num_id).copied();
                                let last_index = content.len();
                                let last_is_same_list = content
                                    .get(last_index.saturating_sub(1))
                                    .and_then(Value::as_object)
                                    .filter(|previous| {
                                        previous.get("type").and_then(Value::as_str)
                                            == Some(list_type.as_str())
                                    })
                                    .is_some();
                                if last_is_same_list {
                                    if let Some(items) = content
                                        .get_mut(last_index - 1)
                                        .and_then(Value::as_object_mut)
                                        .and_then(|previous| previous.get_mut("content"))
                                        .and_then(Value::as_array_mut)
                                    {
                                        items.push(
                                            json!({ "type": "list_item", "content": [node] }),
                                        );
                                    }
                                } else {
                                    let mut list_attrs = Map::new();
                                    if let Some(order) = list_order {
                                        list_attrs.insert("start".to_string(), json!(order));
                                    }
                                    let list = json!({
                                        "type": list_type,
                                        "attrs": Value::Object(list_attrs),
                                        "content": [{ "type": "list_item", "content": [node] }],
                                    });
                                    content.push(list);
                                }
                            } else {
                                content.push(node);
                            }
                        } else if let Some(num_id) = paragraph_num_id {
                            let list_type = list_type_for_num_id(num_id, &num_formats);
                            let list_order = num_starts.get(&num_id).copied();
                            let list_item = json!({ "type": "list_item", "content": [node] });
                            if let Some(open) = &mut open_list {
                                if open.num_id == num_id && open.list_type == list_type {
                                    open.items.push(list_item);
                                } else {
                                    flush_open_list(&mut open_list, &mut paragraphs);
                                    open_list = Some(OpenList {
                                        list_type,
                                        num_id,
                                        items: vec![list_item],
                                        order: list_order,
                                    });
                                }
                            } else {
                                open_list = Some(OpenList {
                                    list_type,
                                    num_id,
                                    items: vec![list_item],
                                    order: list_order,
                                });
                            }
                        } else {
                            flush_open_list(&mut open_list, &mut paragraphs);
                            paragraphs.push(node);
                        }
                    }
                    if pending_page_break {
                        if let Some(content) = cell_content.as_mut() {
                            content.push(json!({ "type": "page_break" }));
                        } else {
                            flush_open_list(&mut open_list, &mut paragraphs);
                            paragraphs.push(json!({ "type": "page_break" }));
                        }
                        pending_page_break = false;
                    }
                }
                b"w:tc" => {
                    let mut attrs = Map::new();
                    if cell_grid_span > 1 {
                        attrs.insert("colspan".to_string(), json!(cell_grid_span));
                    }
                    if cell_vmerge_restart {
                        attrs.insert("vmerge".to_string(), json!("restart"));
                    } else if cell_vmerge_continue {
                        attrs.insert("vmerge".to_string(), json!("continue"));
                    }
                    let mut cell = json!({
                        "type": "table_cell",
                        "content": cell_content.take().unwrap_or_default()
                    });
                    if !attrs.is_empty() {
                        cell["attrs"] = Value::Object(attrs);
                    }
                    if let Some(cells) = row_cells.as_mut() {
                        cells.push(cell);
                    }
                    // Restore the enclosing cell's content after a nested
                    // table finished inside it.
                    cell_content = cell_content_stack.pop();
                    cell_grid_span = 1;
                    cell_vmerge_restart = false;
                    cell_vmerge_continue = false;
                }
                b"w:tblGrid" => in_table_grid = false,
                b"w:tr" => {
                    let mut cells = row_cells.take().unwrap_or_default();
                    if row_is_header {
                        for cell in cells.iter_mut() {
                            if cell.get("type").and_then(Value::as_str) == Some("table_cell") {
                                cell["type"] = json!("table_header");
                                if let Some(attrs) = cell.get("attrs").and_then(Value::as_object) {
                                    let mut header_attrs = attrs.clone();
                                    let _ = header_attrs.remove("vmerge");
                                    if header_attrs.is_empty() {
                                        if let Some(obj) = cell.as_object_mut() {
                                            obj.remove("attrs");
                                        }
                                    } else {
                                        cell["attrs"] = Value::Object(header_attrs);
                                    }
                                }
                            }
                        }
                    }
                    let row = json!({ "type": "table_row", "content": cells });
                    if let Some(rows) = table_stack.last_mut() {
                        rows.push(row);
                    }
                    // Restore the enclosing row's partial cells after a
                    // nested table row finished.
                    row_cells = row_cells_stack.pop();
                    row_is_header = false;
                }
                b"w:tbl" => {
                    let rows = table_stack.pop().ok_or_else(|| {
                        ExportError::Docx(
                            "Unexpected end of table: missing table rows stack".to_string(),
                        )
                    })?;
                    let grid_widths = table_grid_widths_stack.pop().unwrap_or_default();
                    // Attach tblGrid widths (px) to first-row cells so column
                    // sizing survives the DOCX round-trip.
                    let mut rows = rows;
                    if !grid_widths.is_empty() {
                        if let Some(first_row) = rows.first_mut() {
                            let mut cursor = 0usize;
                            if let Some(cells) =
                                first_row.get_mut("content").and_then(Value::as_array_mut)
                            {
                                for cell in cells.iter_mut() {
                                    let span = cell
                                        .get("attrs")
                                        .and_then(|attrs| attrs.get("colspan"))
                                        .and_then(Value::as_u64)
                                        .unwrap_or(1)
                                        as usize;
                                    let covered: Vec<usize> = (cursor
                                        ..(cursor + span).min(grid_widths.len()))
                                        .map(|i| grid_widths[i])
                                        .collect();
                                    if !covered.is_empty() {
                                        if let Some(obj) = cell.as_object_mut() {
                                            let attrs =
                                                obj.entry("attrs").or_insert_with(|| json!({}));
                                            if let Some(map) = attrs.as_object_mut() {
                                                map.insert("colwidth".to_string(), json!(covered));
                                            }
                                        }
                                    }
                                    cursor += span;
                                    if cursor >= grid_widths.len() {
                                        break;
                                    }
                                }
                            }
                        }
                    }
                    let table = json!({ "type": "table", "content": rows });
                    if let Some(content) = cell_content.as_mut() {
                        // Nested table: keep it inside its cell.
                        content.push(table);
                    } else {
                        paragraphs.push(table);
                    }
                }
                _ => {}
            },
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => return Err(ExportError::Docx(error.to_string())),
        }
        buffer.clear();
    }

    flush_open_list(&mut open_list, &mut paragraphs);

    let mut raw_document = json!({ "type": "doc", "content": paragraphs });
    if let Some(page_setup) = imported_page_setup {
        raw_document["pageSetup"] = page_setup;
    }
    let mut comments = Vec::new();
    for (id, comment) in imported_comments {
        let Some((from, to)) = comment_ranges.get(&id).copied() else {
            warnings.push(format!(
                "DOCX comment {id} had no usable anchor and was skipped"
            ));
            continue;
        };
        if from >= to {
            warnings.push(format!(
                "DOCX comment {id} had an empty anchor and was skipped"
            ));
            continue;
        }
        comments.push(json!({
            "id": format!("comment-{id}"),
            "author": comment.author,
            "text": comment.text,
            "from": from,
            "to": to,
            "resolved": false,
            "createdAt": comment.date,
        }));
    }
    if !comments.is_empty() {
        raw_document["comments"] = Value::Array(comments);
    }
    if !imported_footnotes.is_empty() {
        // Preserve footnotes.xml order (word ids ascending).
        let mut ordered: Vec<(&u32, &Value)> = imported_footnotes.iter().collect();
        ordered.sort_by_key(|(word_id, _)| **word_id);
        raw_document["footnotes"] =
            Value::Array(ordered.into_iter().map(|(_, note)| note.clone()).collect());
    }
    let document = redoc_doc_engine::prune_doc(&raw_document);

    Ok(DocxImportResult { document, warnings })
}

pub fn import_docx_to_doc(path: &Path) -> Result<Value, ExportError> {
    Ok(import_docx_to_doc_with_report(path)?.document)
}

fn decode_data_uri(src: &str) -> Option<Vec<u8>> {
    let encoded = src.strip_prefix("data:")?.split_once(',')?.1;
    decode_base64(encoded)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn rejects_unsafe_docx_relationship_targets() {
        assert_eq!(docx_part_path("../../outside.xml"), None);
        assert_eq!(docx_part_path("/word/../outside.xml"), None);
        assert_eq!(
            docx_part_path("media/image.png"),
            Some("word/media/image.png".to_string())
        );
    }

    #[test]
    fn imports_exported_docx_paragraphs_and_marks() {
        let path = std::env::temp_dir().join(format!("redoc-docx-{}.docx", std::process::id()));
        let source = json!({ "type": "doc", "content": [
            { "type": "paragraph", "content": [{ "type": "text", "text": "Hello", "marks": [{ "type": "bold" }] }] },
            { "type": "heading", "attrs": { "level": 6 }, "content": [{ "type": "text", "text": "Deep heading" }] },
            { "type": "page_break" }
        ] });
        std::fs::write(
            &path,
            export_doc_to_docx(&source, "Test").expect("export docx"),
        )
        .expect("write docx");
        let imported = import_docx_to_doc(&path).expect("import docx");
        assert_eq!(imported["content"][0]["content"][0]["text"], "Hello");
        assert_eq!(
            imported["content"][0]["content"][0]["marks"][0]["type"],
            "bold"
        );
        assert_eq!(imported["content"][1]["type"], "heading");
        assert_eq!(imported["content"][1]["attrs"]["level"], 6);
        assert_eq!(imported["content"][2]["type"], "page_break");
        std::fs::remove_file(path).expect("cleanup docx");
    }

    #[test]
    fn round_trips_named_paragraph_styles() {
        let path = std::env::temp_dir().join(format!(
            "redoc-docx-styles-{}-{}.docx",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let source = json!({ "type": "doc", "content": [
            { "type": "paragraph", "attrs": { "styleName": "Title" }, "content": [{ "type": "text", "text": "Report" }] },
            { "type": "paragraph", "attrs": { "styleName": "Quote" }, "content": [{ "type": "text", "text": "Quoted line" }] },
            { "type": "paragraph", "content": [{ "type": "text", "text": "Body" }] }
        ] });
        std::fs::write(
            &path,
            export_doc_to_docx(&source, "Styles Test").expect("export docx"),
        )
        .expect("write docx");
        let imported = import_docx_to_doc(&path).expect("import docx");
        assert_eq!(imported["content"][0]["attrs"]["styleName"], "Title");
        assert_eq!(imported["content"][1]["attrs"]["styleName"], "Quote");
        // Normal paragraphs import with a null styleName, not "Normal".
        assert!(imported["content"][2]["attrs"]["styleName"].is_null());
        std::fs::remove_file(path).expect("cleanup docx");
    }

    #[test]
    fn round_trips_native_footnotes() {
        let path = std::env::temp_dir().join(format!(
            "redoc-docx-footnotes-{}-{}.docx",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let source = json!({
            "type": "doc",
            "content": [
                { "type": "paragraph", "content": [
                    { "type": "text", "text": "Claim one" },
                    { "type": "footnote_ref", "attrs": { "id": "fn-a", "label": 1 } },
                    { "type": "text", "text": " and claim two" },
                    { "type": "footnote_ref", "attrs": { "id": "fn-b", "label": 2 } }
                ] }
            ],
            "footnotes": [
                { "id": "fn-a", "label": 1, "text": "First source." },
                { "id": "fn-b", "label": 2, "text": "Second source & detail." }
            ]
        });
        let bytes = export_doc_to_docx(&source, "Footnotes Test").expect("export docx");
        {
            let mut archive =
                zip::ZipArchive::new(std::io::Cursor::new(&bytes)).expect("read docx zip");
            let mut footnotes_xml = String::new();
            archive
                .by_name("word/footnotes.xml")
                .expect("footnotes part")
                .read_to_string(&mut footnotes_xml)
                .expect("read footnotes");
            assert!(footnotes_xml.contains("First source."));
            assert!(footnotes_xml.contains("Second source &amp; detail."));
            let mut document = String::new();
            archive
                .by_name("word/document.xml")
                .expect("document part")
                .read_to_string(&mut document)
                .expect("read document");
            assert!(document.contains("<w:footnoteReference w:id=\"2\"/>"));
            assert!(document.contains("<w:footnoteReference w:id=\"3\"/>"));
            assert!(!document.contains("[[FN:"));
        }
        std::fs::write(&path, bytes).expect("write docx");

        let imported = import_docx_to_doc(&path).expect("import docx");
        let notes = imported["footnotes"].as_array().expect("footnotes array");
        assert_eq!(notes.len(), 2);
        assert_eq!(notes[0]["id"], "fn-docx-2");
        assert_eq!(notes[0]["text"], "First source.");
        assert_eq!(notes[1]["text"], "Second source & detail.");
        let body = imported["content"][0]["content"]
            .as_array()
            .expect("paragraph content");
        let refs: Vec<&Value> = body
            .iter()
            .filter(|n| n["type"] == "footnote_ref")
            .collect();
        assert_eq!(refs.len(), 2);
        assert_eq!(refs[0]["attrs"]["label"], 1);
        assert_eq!(refs[1]["attrs"]["label"], 2);
        std::fs::remove_file(path).expect("cleanup docx");
    }

    #[test]
    fn round_trips_docx_page_setup_and_header_footer_fields() {
        let source = json!({
            "type": "doc",
            "pageSetup": {
                "margins": { "top": 0.75, "bottom": 0.6, "left": 1.25, "right": 0.8 },
                "orientation": "landscape",
                "paperSize": "legal",
                "header": "Quarterly — Page {page} of {pages}",
                "footer": "Confidential {total}"
            },
            "content": [{ "type": "paragraph", "content": [{ "type": "text", "text": "Body" }] }]
        });
        let bytes = export_doc_to_docx(&source, "Page setup").expect("export docx");
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).expect("read docx zip");
        let mut document_xml = String::new();
        archive
            .by_name("word/document.xml")
            .expect("document xml")
            .read_to_string(&mut document_xml)
            .expect("read document xml");
        assert!(document_xml.contains("w:w=\"20160\""));
        assert!(document_xml.contains("w:h=\"12240\""));
        assert!(document_xml.contains("w:orient=\"landscape\""));
        assert!(document_xml.contains("w:top=\"1080\""));
        assert!(document_xml.contains("w:left=\"1800\""));
        let mut header_xml = String::new();
        archive
            .by_name("word/header1.xml")
            .expect("header xml")
            .read_to_string(&mut header_xml)
            .expect("read header xml");
        assert!(header_xml.contains("Quarterly"));
        assert!(header_xml.contains("PAGE"));
        assert!(header_xml.contains("NUMPAGES"));
        let mut footer_xml = String::new();
        archive
            .by_name("word/footer1.xml")
            .expect("footer xml")
            .read_to_string(&mut footer_xml)
            .expect("read footer xml");
        assert!(footer_xml.contains("Confidential"));
        assert!(footer_xml.contains("NUMPAGES"));

        let path =
            std::env::temp_dir().join(format!("redoc-docx-page-setup-{}.docx", std::process::id()));
        std::fs::write(
            &path,
            export_doc_to_docx(&source, "Page setup").expect("export docx"),
        )
        .expect("write docx");
        let imported = import_docx_to_doc(&path).expect("import docx");
        assert_eq!(imported["pageSetup"]["paperSize"], "legal");
        assert_eq!(imported["pageSetup"]["orientation"], "landscape");
        assert_eq!(imported["pageSetup"]["margins"]["left"], 1.25);
        assert_eq!(
            imported["pageSetup"]["header"],
            "Quarterly — Page {page} of {pages}"
        );
        assert_eq!(imported["pageSetup"]["footer"], "Confidential {pages}");
        std::fs::remove_file(path).expect("cleanup docx");
    }

    #[test]
    fn section_break_exports_native_sectpr_not_dropped() {
        // Regression: section_break nodes used to be silently omitted from
        // DOCX (and whitelisted as safe), losing real user data.
        let source = json!({
            "type": "doc",
            "content": [
                { "type": "paragraph", "content": [{ "type": "text", "text": "First section" }] },
                { "type": "section_break", "attrs": { "pageSetup": {
                    "paperSize": "a4",
                    "orientation": "landscape",
                    "columns": 1,
                    "margins": { "top": 0.75, "bottom": 0.75, "left": 1.0, "right": 1.0 }
                } } },
                { "type": "paragraph", "content": [{ "type": "text", "text": "Second section" }] }
            ]
        });
        let bytes = export_doc_to_docx(&source, "Sections").expect("export docx");
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).expect("read zip");
        let mut document_xml = String::new();
        archive
            .by_name("word/document.xml")
            .expect("document xml")
            .read_to_string(&mut document_xml)
            .expect("read xml");
        // A sectPr mid-document (not just the trailing body sectPr): the
        // break must appear BEFORE the "Second section" text.
        let break_pos = document_xml
            .find("w:sectPr")
            .expect("section break sectPr emitted");
        let second_pos = document_xml
            .find("Second section")
            .expect("second section text");
        assert!(
            break_pos < second_pos,
            "sectPr must precede following content"
        );
        assert!(document_xml.contains("w:orient=\"landscape\""));
        assert!(document_xml.contains("w:w=\"16838\""));
        // Both sections' text is preserved.
        assert!(document_xml.contains("First section"));
        assert!(document_xml.contains("Second section"));
    }

    #[test]
    fn round_trips_docx_section_columns() {
        let source = json!({
            "type": "doc",
            "pageSetup": {
                "columns": 3,
                "margins": { "top": 0.75, "bottom": 0.75, "left": 0.75, "right": 0.75 }
            },
            "content": [{ "type": "paragraph", "content": [{ "type": "text", "text": "Column body" }] }]
        });
        let bytes = export_doc_to_docx(&source, "Columns").expect("export columns");
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).expect("read docx zip");
        let mut document_xml = String::new();
        archive
            .by_name("word/document.xml")
            .expect("document xml")
            .read_to_string(&mut document_xml)
            .expect("read document xml");
        assert!(document_xml.contains("<w:cols") && document_xml.contains("w:num=\"3\""));

        let path =
            std::env::temp_dir().join(format!("redoc-docx-columns-{}.docx", std::process::id()));
        std::fs::write(
            &path,
            export_doc_to_docx(&source, "Columns").expect("export columns"),
        )
        .expect("write docx");
        let imported = import_docx_to_doc(&path).expect("import columns");
        std::fs::remove_file(path).expect("cleanup docx");
        assert_eq!(imported["pageSetup"]["columns"], 3);
    }

    #[test]
    fn exports_review_comments_with_native_docx_anchors() {
        let source = json!({
            "type": "doc",
            "content": [{
                "type": "paragraph",
                "content": [{ "type": "text", "text": "Hello world" }]
            }],
            "comments": [{
                "id": "comment-1",
                "author": "Reviewer",
                "text": "Check this greeting",
                "from": 1,
                "to": 6,
                "resolved": false,
                "createdAt": "2026-08-31T00:00:00Z"
            }]
        });
        let bytes = export_doc_to_docx(&source, "Commented").expect("export docx");
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).expect("read docx zip");
        let mut comments_xml = String::new();
        archive
            .by_name("word/comments.xml")
            .expect("comments xml")
            .read_to_string(&mut comments_xml)
            .expect("read comments xml");
        assert!(comments_xml.contains("Reviewer"));
        assert!(comments_xml.contains("Check this greeting"));
        let mut document_xml = String::new();
        archive
            .by_name("word/document.xml")
            .expect("document xml")
            .read_to_string(&mut document_xml)
            .expect("read document xml");
        assert!(document_xml.contains("commentRangeStart"));
        assert!(document_xml.contains("commentRangeEnd"));
    }

    #[test]
    fn imports_native_docx_comments_into_review_metadata() {
        let source = json!({
            "type": "doc",
            "content": [{
                "type": "paragraph",
                "content": [{ "type": "text", "text": "Hello world" }]
            }],
            "comments": [{
                "id": "comment-1",
                "author": "Reviewer",
                "text": "Check this greeting",
                "from": 1,
                "to": 6,
                "resolved": false,
                "createdAt": "2026-08-31T00:00:00Z"
            }]
        });
        let path = std::env::temp_dir().join(format!(
            "redoc-docx-comments-{}-{}.docx",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::write(
            &path,
            export_doc_to_docx(&source, "Commented").expect("export docx"),
        )
        .expect("write docx");
        let imported = import_docx_to_doc_with_report(&path).expect("import docx");
        let comment = imported.document["comments"][0].clone();
        assert_eq!(comment["author"], "Reviewer");
        assert_eq!(comment["text"], "Check this greeting");
        assert_eq!(comment["from"], 1);
        assert_eq!(comment["to"], 6);
        std::fs::remove_file(path).expect("cleanup docx");
    }

    #[test]
    fn reports_unsupported_docx_constructs() {
        let path =
            std::env::temp_dir().join(format!("redoc-docx-report-{}.docx", std::process::id()));
        let cursor = std::io::Cursor::new(Vec::new());
        let mut archive = zip::ZipWriter::new(cursor);
        archive
            .start_file(
                "word/document.xml",
                zip::write::SimpleFileOptions::default(),
            )
            .expect("start document");
        archive
            .write_all(br#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:hyperlink><w:p><w:r><w:t>Link</w:t></w:r></w:p></w:hyperlink></w:body></w:document>"#)
            .expect("write document");
        let bytes = archive.finish().expect("finish docx").into_inner();
        std::fs::write(&path, bytes).expect("write docx");
        let report = import_docx_to_doc_with_report(&path).expect("import docx");
        assert_eq!(report.document["content"][0]["content"][0]["text"], "Link");
        assert!(!report
            .warnings
            .iter()
            .any(|warning| warning.contains("Hyperlinks")));
        std::fs::remove_file(path).expect("cleanup docx");
    }

    #[test]
    fn imports_native_tracked_changes_as_review_marks() {
        let path =
            std::env::temp_dir().join(format!("redoc-docx-tracked-{}.docx", std::process::id()));
        let cursor = std::io::Cursor::new(Vec::new());
        let mut archive = zip::ZipWriter::new(cursor);
        archive
            .start_file(
                "word/document.xml",
                zip::write::SimpleFileOptions::default(),
            )
            .expect("start document");
        archive
            .write_all(br#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>Keep </w:t></w:r><w:ins w:id="7" w:author="Alice" w:date="2026-08-31T10:00:00Z"><w:r><w:t>new</w:t></w:r></w:ins><w:del w:id="8" w:author="Bob"><w:r><w:delText>old</w:delText></w:r></w:del></w:p></w:body></w:document>"#)
            .expect("write document");
        let bytes = archive.finish().expect("finish docx").into_inner();
        std::fs::write(&path, bytes).expect("write docx");

        let report = import_docx_to_doc_with_report(&path).expect("import docx");
        let runs = report.document["content"][0]["content"].as_array().unwrap();
        assert_eq!(runs.len(), 3);
        assert_eq!(runs[1]["marks"][0]["type"], "trackInsert");
        assert_eq!(runs[1]["marks"][0]["attrs"]["author"], "Alice");
        assert_eq!(runs[2]["marks"][0]["type"], "trackDelete");
        assert_eq!(runs[2]["marks"][0]["attrs"]["author"], "Bob");
        assert!(!report
            .warnings
            .iter()
            .any(|warning| warning.contains("Tracked changes were skipped")));
        std::fs::remove_file(path).expect("cleanup docx");
    }

    #[test]
    fn exports_tracked_changes_as_native_docx_revision_elements() {
        let source = json!({
            "type": "doc",
            "content": [{
                "type": "paragraph",
                "content": [
                    { "type": "text", "text": "added", "marks": [{ "type": "trackInsert", "attrs": { "author": "Alice", "createdAt": "2026-08-31T10:00:00Z" } }] },
                    { "type": "text", "text": "removed", "marks": [{ "type": "trackDelete", "attrs": { "author": "Bob", "createdAt": "2026-08-31T11:00:00Z" } }] }
                ]
            }]
        });
        let bytes = export_doc_to_docx(&source, "Tracked").expect("export docx");
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).expect("read docx zip");
        let mut document_xml = String::new();
        archive
            .by_name("word/document.xml")
            .expect("document xml")
            .read_to_string(&mut document_xml)
            .expect("read document xml");
        assert!(document_xml.contains("w:ins"));
        assert!(document_xml.contains("w:del"));
        assert!(document_xml.contains("w:delText"));
        assert!(document_xml.contains("Alice"));
        assert!(document_xml.contains("Bob"));
    }

    #[test]
    fn rejects_docx_archives_with_too_many_entries() {
        let path = std::env::temp_dir().join(format!(
            "redoc-docx-entry-limit-{}.docx",
            std::process::id()
        ));
        let file = std::fs::File::create(&path).expect("create fixture");
        let mut archive = zip::ZipWriter::new(file);
        for index in 0..=MAX_DOCX_ARCHIVE_ENTRIES {
            archive
                .start_file(
                    format!("word/extra-{index}.xml"),
                    zip::write::SimpleFileOptions::default(),
                )
                .expect("start entry");
        }
        archive.finish().expect("finish fixture");
        let error = import_docx_to_doc_with_report(&path).expect_err("entry limit should fail");
        assert!(error.to_string().contains("too many entries"));
        std::fs::remove_file(path).expect("cleanup docx");
    }

    #[test]
    fn exports_simple_tables() {
        let source = json!({
            "type": "doc",
            "content": [{
                "type": "table",
                "content": [{
                    "type": "table_row",
                    "content": [{ "type": "table_cell", "content": [{ "type": "paragraph", "content": [{ "type": "text", "text": "Cell" }] }] }]
                }]
            }]
        });
        let bytes = export_doc_to_docx(&source, "Table").expect("export table");
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).expect("read docx");
        let mut document = String::new();
        archive
            .by_name("word/document.xml")
            .expect("document part")
            .read_to_string(&mut document)
            .expect("read document");
        assert!(document.contains("<w:tbl>"));
        assert!(document.contains("Cell"));
    }

    #[test]
    fn imports_simple_tables() {
        let path =
            std::env::temp_dir().join(format!("redoc-docx-table-{}.docx", std::process::id()));
        let source = json!({
            "type": "doc",
            "content": [{
                "type": "table",
                "content": [{
                    "type": "table_row",
                    "content": [
                        { "type": "table_cell", "content": [{ "type": "paragraph", "content": [{ "type": "text", "text": "A1" }] }] },
                        { "type": "table_cell", "content": [{ "type": "paragraph", "content": [{ "type": "text", "text": "B1" }] }] }
                    ]
                }]
            }]
        });
        std::fs::write(
            &path,
            export_doc_to_docx(&source, "Table").expect("export docx"),
        )
        .expect("write docx");
        let imported = import_docx_to_doc(&path).expect("import docx");
        assert_eq!(imported["content"][0]["type"], "table");
        assert_eq!(imported["content"][0]["content"][0]["type"], "table_row");
        assert_eq!(
            imported["content"][0]["content"][0]["content"][1]["type"],
            "table_cell"
        );
        assert_eq!(
            imported["content"][0]["content"][0]["content"][1]["content"][0]["content"][0]["text"],
            "B1"
        );
        std::fs::remove_file(path).expect("cleanup docx");
    }

    #[test]
    fn imports_nested_tables_structurally() {
        let path =
            std::env::temp_dir().join(format!("redoc-docx-nested-{}.docx", std::process::id()));
        let file = std::fs::File::create(&path).expect("create fixture");
        let mut archive = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default();
        archive
            .start_file("word/document.xml", options)
            .expect("document entry");
        std::io::Write::write_all(
            &mut archive,
            br#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>
<w:tbl><w:tr><w:tc><w:p><w:r><w:t>Outer cell</w:t></w:r></w:p>
<w:tbl><w:tr><w:tc><w:p><w:r><w:t>Inner cell</w:t></w:r></w:p></w:tc></w:tr></w:tbl>
</w:tc></w:tr></w:tbl>
</w:body></w:document>"#,
        )
        .expect("document xml");
        archive.finish().expect("finish fixture");

        let imported = import_docx_to_doc(&path).expect("import nested table");
        let outer = &imported["content"][0];
        assert_eq!(outer["type"], "table");
        let inner = &outer["content"][0]["content"][0]["content"][1];
        // The inner table must land INSIDE the outer cell, not flattened into
        // the top-level paragraph stream.
        assert_eq!(inner["type"], "table");
        assert_eq!(
            inner["content"][0]["content"][0]["content"][0]["content"][0]["text"],
            "Inner cell"
        );
        assert_eq!(
            outer["content"][0]["content"][0]["content"][0]["content"][0]["text"],
            "Outer cell"
        );
        std::fs::remove_file(path).expect("cleanup docx");
    }

    #[test]
    fn imports_tblgrid_column_widths() {
        let path =
            std::env::temp_dir().join(format!("redoc-docx-tblgrid-{}.docx", std::process::id()));
        let file = std::fs::File::create(&path).expect("create fixture");
        let mut archive = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default();
        archive
            .start_file("word/document.xml", options)
            .expect("document entry");
        std::io::Write::write_all(
            &mut archive,
            br#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>
<w:tbl><w:tblGrid><w:gridCol w:w="2400"/><w:gridCol w:w="3600"/></w:tblGrid>
<w:tr><w:tc><w:p/></w:tc><w:tc><w:p/></w:tc></w:tr></w:tbl>
</w:body></w:document>"#,
        )
        .expect("document xml");
        archive.finish().expect("finish fixture");

        let imported = import_docx_to_doc(&path).expect("import tblgrid");
        let cells = &imported["content"][0]["content"][0]["content"];
        assert_eq!(
            cells[0]["attrs"]["colwidth"],
            json!([160]), // 2400 twips / 15
        );
        assert_eq!(cells[1]["attrs"]["colwidth"], json!([240])); // 3600 / 15
        std::fs::remove_file(path).expect("cleanup docx");
    }

    #[test]
    fn imports_embedded_images_as_data_uris() {
        let path =
            std::env::temp_dir().join(format!("redoc-docx-image-{}.docx", std::process::id()));
        let file = std::fs::File::create(&path).expect("create fixture");
        let mut archive = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default();
        archive
            .start_file("word/document.xml", options)
            .expect("document entry");
        std::io::Write::write_all(
            &mut archive,
            br#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><w:body><w:p><w:r><w:drawing><a:blip r:embed="rId5"/></w:drawing></w:r></w:p></w:body></w:document>"#,
        ).expect("document xml");
        archive
            .start_file("word/_rels/document.xml.rels", options)
            .expect("rels entry");
        std::io::Write::write_all(
            &mut archive,
            br#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId5" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/picture.png"/></Relationships>"#,
        ).expect("rels xml");
        archive
            .start_file("word/media/picture.png", options)
            .expect("media entry");
        std::io::Write::write_all(&mut archive, b"png-bytes").expect("media bytes");
        archive.finish().expect("finish fixture");

        let imported = import_docx_to_doc(&path).expect("import image");
        assert_eq!(imported["content"][0]["content"][0]["type"], "image");
        assert_eq!(
            imported["content"][0]["content"][0]["attrs"]["src"],
            "data:image/png;base64,cG5nLWJ5dGVz"
        );
        std::fs::remove_file(path).expect("cleanup docx");
    }

    #[test]
    fn exports_data_uri_images() {
        let source = json!({
            "type": "doc",
            "content": [{
                "type": "paragraph",
                "content": [{
                    "type": "image",
                    "attrs": {
                        "src": "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=",
                        "alt": "pixel"
                    }
                }]
            }]
        });
        let bytes = export_doc_to_docx(&source, "Image").expect("export image");
        let archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).expect("read docx");
        let names = archive.file_names().map(str::to_string).collect::<Vec<_>>();
        // docx-rs mints image rIds from a process-global counter, so the
        // exact index varies with test order; only the media part matters.
        assert!(
            names
                .iter()
                .any(|name| name.starts_with("word/media/rIdImage")),
            "media entries: {names:?}"
        );
    }

    #[test]
    fn exports_code_block_with_font_and_shading() {
        let source = json!({
            "type": "doc",
            "content": [{
                "type": "code_block",
                "content": [{ "type": "text", "text": "fn main() {\n    println!(\"hello\");\n}" }]
            }]
        });
        let bytes = export_doc_to_docx(&source, "Code").expect("export code_block");
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).expect("read docx");
        let mut document = String::new();
        archive
            .by_name("word/document.xml")
            .expect("document part")
            .read_to_string(&mut document)
            .expect("read document");
        assert!(document.contains("Courier New"));
        assert!(document.contains("shd"));
        assert!(document.contains("fn main()"));
    }

    #[test]
    fn exports_list_num_pr() {
        let source = json!({
            "type": "doc",
            "content": [{
                "type": "bullet_list",
                "content": [{
                    "type": "list_item",
                    "content": [{
                        "type": "paragraph",
                        "content": [{ "type": "text", "text": "Item 1" }]
                    }]
                }]
            }]
        });
        let bytes = export_doc_to_docx(&source, "List").expect("export list");
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).expect("read docx");
        let mut document = String::new();
        archive
            .by_name("word/document.xml")
            .expect("document part")
            .read_to_string(&mut document)
            .expect("read document");
        assert!(document.contains("numPr"));
        assert!(document.contains("numId"));
    }

    #[test]
    fn exports_hyperlinks() {
        let source = json!({
            "type": "doc",
            "content": [{
                "type": "paragraph",
                "content": [{
                    "type": "text",
                    "text": "Antigravity",
                    "marks": [{ "type": "link", "attrs": { "href": "https://example.com" } }]
                }]
            }]
        });
        let bytes = export_doc_to_docx(&source, "Link").expect("export hyperlink");
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).expect("read docx");
        let mut document = String::new();
        archive
            .by_name("word/document.xml")
            .expect("document part")
            .read_to_string(&mut document)
            .expect("read document");
        assert!(document.contains("hyperlink"));
    }

    #[test]
    fn rejects_javascript_and_data_hyperlinks() {
        assert!(super::safe_doc_hyperlink("javascript:alert(1)").is_none());
        assert!(super::safe_doc_hyperlink("data:text/html,hi").is_none());
        assert!(super::safe_doc_hyperlink("vbscript:msgbox(1)").is_none());
        assert_eq!(
            super::safe_doc_hyperlink("https://example.com").as_deref(),
            Some("https://example.com")
        );
        assert_eq!(
            super::safe_doc_hyperlink("#Heading 1").as_deref(),
            Some("internal:Heading 1")
        );
    }

    #[test]
    fn imports_hyperlinks_as_link_marks_without_warning() {
        let source = json!({
            "type": "doc",
            "content": [{
                "type": "paragraph",
                "content": [{
                    "type": "text",
                    "text": "Open site",
                    "marks": [{ "type": "link", "attrs": { "href": "https://example.com" } }]
                }]
            }]
        });
        let path =
            std::env::temp_dir().join(format!("redoc-docx-hyperlink-{}.docx", std::process::id()));
        std::fs::write(
            &path,
            export_doc_to_docx(&source, "Link").expect("export hyperlink"),
        )
        .expect("write docx");
        let result = import_docx_to_doc_with_report(&path).expect("import hyperlink");
        std::fs::remove_file(path).expect("cleanup docx");

        let imported_text = &result.document["content"][0]["content"][0];
        assert_eq!(imported_text["text"], "Open site");
        let link_mark = imported_text["marks"]
            .as_array()
            .and_then(|marks| marks.iter().find(|mark| mark["type"] == "link"))
            .expect("hyperlink should remain editable as a link mark");
        assert_eq!(link_mark["attrs"]["href"], "https://example.com");
        assert!(!result
            .warnings
            .iter()
            .any(|warning| warning.to_lowercase().contains("hyperlink")));
    }

    #[test]
    fn exports_merged_table_cells() {
        let source = json!({
            "type": "doc",
            "content": [{
                "type": "table",
                "content": [{
                    "type": "table_row",
                    "content": [{
                        "type": "table_cell",
                        "attrs": { "colspan": 2, "rowspan": 2 },
                        "content": [{ "type": "paragraph", "content": [{ "type": "text", "text": "Merged" }] }]
                    }]
                }]
            }]
        });
        let bytes = export_doc_to_docx(&source, "Table").expect("export merged table");
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).expect("read docx");
        let mut document = String::new();
        archive
            .by_name("word/document.xml")
            .expect("document part")
            .read_to_string(&mut document)
            .expect("read document");
        assert!(document.contains("gridSpan"));
        assert!(document.contains("vMerge"));
    }

    #[test]
    fn round_trips_bookmarks_and_internal_hyperlinks() {
        let source = json!({
            "type": "doc",
            "content": [
                { "type": "paragraph", "content": [
                    { "type": "text", "text": "Jump to ", "marks": [] },
                    { "type": "text", "text": "target", "marks": [
                        { "type": "link", "attrs": { "href": "internal:Target" } }
                    ] }
                ] },
                { "type": "paragraph", "content": [
                    { "type": "text", "text": "Here", "marks": [
                        { "type": "bookmark", "attrs": { "name": "Target" } }
                    ] }
                ] }
            ]
        });
        let bytes = export_doc_to_docx(&source, "Bookmarks").expect("export bookmarks");
        {
            let mut archive =
                zip::ZipArchive::new(std::io::Cursor::new(&bytes)).expect("read docx zip");
            let mut document = String::new();
            archive
                .by_name("word/document.xml")
                .expect("document part")
                .read_to_string(&mut document)
                .expect("read document");
            assert!(
                document.contains("<w:bookmarkStart"),
                "bookmarkStart must be present, got: {document}"
            );
            assert!(document.contains("w:name=\"Target\""));
            assert!(document.contains("<w:bookmarkEnd"));
            assert!(
                document.contains("w:anchor=\"Target\""),
                "internal link must export as a w:anchor hyperlink, got: {document}"
            );
        }
        let path = std::env::temp_dir().join(format!(
            "redoc-docx-bookmarks-{}-{}.docx",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::write(&path, bytes).expect("write docx");
        let result = import_docx_to_doc_with_report(&path).expect("import bookmarks");
        std::fs::remove_file(path).expect("cleanup docx");

        let link_run = result.document["content"][0]["content"][1].clone();
        let link_mark = link_run["marks"]
            .as_array()
            .and_then(|marks| marks.iter().find(|mark| mark["type"] == "link"))
            .expect("internal link should import as a link mark");
        assert_eq!(link_mark["attrs"]["href"], "internal:Target");
        let target_run = result.document["content"][1]["content"][0].clone();
        assert!(
            target_run["marks"]
                .as_array()
                .is_some_and(|marks| marks.iter().any(|mark| mark["type"] == "bookmark")),
            "bookmark should import as a bookmark mark"
        );
    }

    #[test]
    fn round_trips_native_page_fields_in_document_body() {
        let source = json!({
            "type": "doc",
            "content": [{
                "type": "paragraph",
                "content": [
                    { "type": "text", "text": "Page " },
                    { "type": "field", "attrs": { "kind": "page", "result": "3" } },
                    { "type": "text", "text": " of " },
                    { "type": "field", "attrs": { "kind": "numPages", "result": "9" } }
                ]
            }]
        });
        let bytes = export_doc_to_docx(&source, "Fields").expect("export fields");
        {
            let mut archive =
                zip::ZipArchive::new(std::io::Cursor::new(&bytes)).expect("read docx zip");
            let mut document = String::new();
            archive
                .by_name("word/document.xml")
                .expect("document part")
                .read_to_string(&mut document)
                .expect("read document");
            assert!(
                document.contains("PAGE"),
                "body field must export native PAGE instruction"
            );
            assert!(document.contains("NUMPAGES"));
            assert!(document.contains("fldChar"));
        }
        let path = std::env::temp_dir().join(format!(
            "redoc-docx-body-fields-{}-{}.docx",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::write(&path, bytes).expect("write docx");
        let imported = import_docx_to_doc(&path).expect("import fields");
        std::fs::remove_file(path).expect("cleanup docx");
        let runs = imported["content"][0]["content"].as_array().unwrap();
        let fields: Vec<&Value> = runs.iter().filter(|node| node["type"] == "field").collect();
        assert_eq!(fields.len(), 2, "both fields must round-trip: {runs:?}");
        assert_eq!(fields[0]["attrs"]["kind"], "page");
        assert_eq!(fields[1]["attrs"]["kind"], "numPages");
        // Cached results are dropped; the editor recomputes them.
        assert!(fields[0]["attrs"]["result"].is_null());
    }

    #[test]
    fn imports_simple_page_field_with_cached_result() {
        let path = std::env::temp_dir().join(format!(
            "redoc-docx-page-field-{}-{}.docx",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let cursor = std::io::Cursor::new(Vec::new());
        let mut archive = zip::ZipWriter::new(cursor);
        let options = zip::write::SimpleFileOptions::default();
        archive
            .start_file("word/document.xml", options)
            .expect("start document");
        std::io::Write::write_all(
            &mut archive,
            br#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>Page </w:t></w:r><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText>PAGE</w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>7</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r></w:p></w:body></w:document>"#,
        )
        .expect("write document");
        let bytes = archive.finish().expect("finish docx").into_inner();
        std::fs::write(&path, bytes).expect("write docx");
        let result = import_docx_to_doc_with_report(&path).expect("import page field");
        std::fs::remove_file(path).expect("cleanup docx");
        let runs = result.document["content"][0]["content"].as_array().unwrap();
        let field = runs
            .iter()
            .find(|node| node["type"] == "field")
            .expect("PAGE field should import as a field node");
        assert_eq!(field["attrs"]["kind"], "page");
        // The cached "7" result text must not leak into the body.
        assert!(
            !runs.iter().any(|node| node["text"] == "7"),
            "cached field result must not leak: {runs:?}"
        );
    }

    #[test]
    fn skips_unsupported_simple_field_with_warning() {
        let path = std::env::temp_dir().join(format!(
            "redoc-docx-unsupported-field-{}-{}.docx",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let cursor = std::io::Cursor::new(Vec::new());
        let mut archive = zip::ZipWriter::new(cursor);
        let options = zip::write::SimpleFileOptions::default();
        archive
            .start_file("word/document.xml", options)
            .expect("start document");
        std::io::Write::write_all(
            &mut archive,
            br#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>Doc </w:t></w:r><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText>TIME \@ "HH:mm"</w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>14:32</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r></w:p></w:body></w:document>"#,
        )
        .expect("write document");
        let bytes = archive.finish().expect("finish docx").into_inner();
        std::fs::write(&path, bytes).expect("write docx");
        let result = import_docx_to_doc_with_report(&path).expect("import time field");
        std::fs::remove_file(path).expect("cleanup docx");
        let runs = result.document["content"][0]["content"].as_array().unwrap();
        assert!(
            !runs.iter().any(|node| node["type"] == "field"),
            "TIME fields are not supported and must not become field nodes: {runs:?}"
        );
        assert!(
            !runs.iter().any(|node| node["text"] == "14:32"),
            "cached unsupported-field result must not leak: {runs:?}"
        );
        assert!(
            result
                .warnings
                .iter()
                .any(|warning| warning.contains("Word fields were skipped")),
            "unsupported fields should warn: {:?}",
            result.warnings
        );
    }

    #[test]
    fn exports_bounded_paragraph_spacing() {
        let source = json!({
            "type": "doc",
            "content": [
                { "type": "paragraph", "attrs": { "spacingBefore": 12, "spacingAfter": 6 }, "content": [{ "type": "text", "text": "Spaced" }] },
                { "type": "paragraph", "attrs": { "spacingBefore": 90_000.0, "spacingAfter": -5.0 }, "content": [{ "type": "text", "text": "Clamped" }] }
            ]
        });
        let bytes = export_doc_to_docx(&source, "Spacing").expect("export spacing");
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).expect("read docx");
        let mut document = String::new();
        archive
            .by_name("word/document.xml")
            .expect("document part")
            .read_to_string(&mut document)
            .expect("read document");
        assert!(
            document.contains("w:before=\"240\""),
            "12pt spacingBefore -> 240 twips, got: {document}"
        );
        assert!(document.contains("w:after=\"120\""));
        // Negative values clamp to 0; oversized values clamp to the Word max.
        assert!(
            !document.contains("w:before=\"-"),
            "spacing must never be negative"
        );
        assert!(document.contains("w:before=\"63360\""));
    }

    #[test]
    fn round_trips_paragraph_layout_attributes() {
        let source = json!({
            "type": "doc",
            "content": [{
                "type": "paragraph",
                "attrs": {
                    "align": "center",
                    "indent": 1.5,
                    "lineHeight": 2.0,
                    "spacingBefore": 6,
                    "spacingAfter": 10
                },
                "content": [{ "type": "text", "text": "Layout" }]
            }]
        });
        let path = std::env::temp_dir().join(format!(
            "redoc-docx-layout-{}-{}.docx",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::write(
            &path,
            export_doc_to_docx(&source, "Layout").expect("export layout"),
        )
        .expect("write docx");
        let imported = import_docx_to_doc(&path).expect("import layout");
        std::fs::remove_file(path).expect("cleanup docx");
        let attrs = &imported["content"][0]["attrs"];
        assert_eq!(attrs["align"], "center");
        assert_eq!(attrs["indent"], 1.5);
        assert_eq!(attrs["lineHeight"], 2.0);
        assert_eq!(attrs["spacingBefore"], 6.0);
        assert_eq!(attrs["spacingAfter"], 10.0);
    }

    #[test]
    fn exports_table_column_widths_and_rectangular_merges() {
        let source = json!({
            "type": "doc",
            "content": [{
                "type": "table",
                "content": [
                    { "type": "table_row", "content": [
                        { "type": "table_cell", "attrs": { "colwidth": [160] }, "content": [{ "type": "paragraph", "content": [{ "type": "text", "text": "A" }] }] },
                        { "type": "table_cell", "attrs": { "colspan": 2, "colwidth": [240, 320] }, "content": [{ "type": "paragraph", "content": [{ "type": "text", "text": "B" }] }] }
                    ] },
                    { "type": "table_row", "content": [
                        { "type": "table_cell", "attrs": { "colspan": 3, "rowspan": 3 }, "content": [{ "type": "paragraph", "content": [{ "type": "text", "text": "Merged" }] }] }
                    ] },
                    { "type": "table_row", "content": [] },
                    { "type": "table_row", "content": [] }
                ]
            }]
        });
        let bytes = export_doc_to_docx(&source, "Grid").expect("export grid");
        let mut archive =
            zip::ZipArchive::new(std::io::Cursor::new(bytes.clone())).expect("read docx");
        let mut document = String::new();
        archive
            .by_name("word/document.xml")
            .expect("document part")
            .read_to_string(&mut document)
            .expect("read document");
        // gridCol widths from colwidth attrs (px * 15 twips at 96dpi).
        assert!(
            document.contains("<w:gridCol w:w=\"2400\""),
            "160px -> 2400 twips gridCol, got: {document}"
        );
        assert!(document.contains("<w:gridCol w:w=\"3600\""));
        assert!(document.contains("<w:gridCol w:w=\"4800\""));
        // Every row covers the same 3-column grid: the two rows below the
        // rowspan=3 Restart cell each carry a colspan-3 vMerge=Continue cell.
        let vmerge_continue = document.matches("w:val=\"continue\"").count();
        assert!(
            vmerge_continue >= 2,
            "covered vMerge=Continue cells must keep rows rectangular, got {vmerge_continue}"
        );
        let tr_count = document.matches("<w:tr>").count();
        assert_eq!(tr_count, 4);
        // Round-trip: import reconstructs colspan / vmerge attrs.
        let path = std::env::temp_dir().join(format!(
            "redoc-docx-grid-{}-{}.docx",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::write(&path, bytes).expect("write docx");
        let imported = import_docx_to_doc(&path).expect("import grid");
        std::fs::remove_file(path).expect("cleanup docx");
        let merged = &imported["content"][0]["content"][1]["content"][0];
        assert_eq!(merged["attrs"]["colspan"], 3);
        assert_eq!(merged["attrs"]["vmerge"], "restart");
    }

    #[test]
    fn round_trips_table_header_rows() {
        let source = json!({
            "type": "doc",
            "content": [{
                "type": "table",
                "content": [
                    { "type": "table_row", "content": [
                        { "type": "table_header", "content": [{ "type": "paragraph", "content": [{ "type": "text", "text": "Head" }] }] },
                        { "type": "table_header", "content": [{ "type": "paragraph", "content": [{ "type": "text", "text": "Cols" }] }] }
                    ] },
                    { "type": "table_row", "content": [
                        { "type": "table_cell", "content": [{ "type": "paragraph", "content": [{ "type": "text", "text": "A1" }] }] },
                        { "type": "table_cell", "content": [{ "type": "paragraph", "content": [{ "type": "text", "text": "B1" }] }] }
                    ] }
                ]
            }]
        });
        let bytes = export_doc_to_docx(&source, "Header").expect("export header table");
        {
            let mut archive =
                zip::ZipArchive::new(std::io::Cursor::new(&bytes)).expect("read docx zip");
            let mut document = String::new();
            archive
                .by_name("word/document.xml")
                .expect("document part")
                .read_to_string(&mut document)
                .expect("read document");
            assert!(
                document.contains("<w:tblHeader/>"),
                "header row must carry w:tblHeader, got: {document}"
            );
        }
        let path = std::env::temp_dir().join(format!(
            "redoc-docx-header-{}-{}.docx",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::write(&path, bytes).expect("write docx");
        let imported = import_docx_to_doc(&path).expect("import header table");
        std::fs::remove_file(path).expect("cleanup docx");
        let cells = imported["content"][0]["content"][0]["content"]
            .as_array()
            .unwrap();
        assert!(
            cells.iter().all(|cell| cell["type"] == "table_header"),
            "tblHeader rows import back as table_header cells: {cells:?}"
        );
    }

    #[test]
    fn exports_ordered_list_start_values() {
        let source = json!({
            "type": "doc",
            "content": [{
                "type": "ordered_list",
                "attrs": { "order": 5 },
                "content": [
                    { "type": "list_item", "content": [{ "type": "paragraph", "content": [{ "type": "text", "text": "Fifth" }] }] },
                    { "type": "list_item", "content": [{ "type": "paragraph", "content": [{ "type": "text", "text": "Sixth" }] }] }
                ]
            }]
        });
        let path = std::env::temp_dir().join(format!(
            "redoc-docx-list-start-{}-{}.docx",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::write(
            &path,
            export_doc_to_docx(&source, "List start").expect("export list start"),
        )
        .expect("write docx");
        {
            let file = std::fs::File::open(&path).expect("reopen docx");
            let mut archive = zip::ZipArchive::new(file).expect("read docx zip");
            let mut numbering = String::new();
            archive
                .by_name("word/numbering.xml")
                .expect("numbering part")
                .read_to_string(&mut numbering)
                .expect("read numbering");
            assert!(
                numbering.contains("w:startOverride w:val=\"5\""),
                "start override must be written, got: {numbering}"
            );
        }
        let imported = import_docx_to_doc(&path).expect("import list start");
        std::fs::remove_file(path).expect("cleanup docx");
        assert_eq!(imported["content"][0]["type"], "ordered_list");
        assert_eq!(
            imported["content"][0]["attrs"]["order"], 5,
            "ordered_list start must round-trip"
        );
    }

    #[test]
    fn exports_comment_thread_replies() {
        let source = json!({
            "type": "doc",
            "content": [{
                "type": "paragraph",
                "content": [{ "type": "text", "text": "Hello world" }]
            }],
            "comments": [{
                "id": "comment-1",
                "author": "Reviewer",
                "text": "Check this",
                "from": 1,
                "to": 6,
                "resolved": false,
                "createdAt": "2026-08-31T00:00:00Z",
                "replies": [{
                    "id": "reply-1",
                    "author": "Author",
                    "text": "Fixed in rev 2",
                    "createdAt": "2026-08-31T01:00:00Z"
                }]
            }]
        });
        let bytes = export_doc_to_docx(&source, "Threaded").expect("export thread");
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).expect("read docx");
        let mut comments_xml = String::new();
        archive
            .by_name("word/comments.xml")
            .expect("comments xml")
            .read_to_string(&mut comments_xml)
            .expect("read comments xml");
        assert!(comments_xml.contains("Check this"));
        assert!(comments_xml.contains("Fixed in rev 2"));
        assert!(comments_xml.contains("Author"));
    }

    #[test]
    fn exports_image_dimensions_from_attrs() {
        let source = json!({
            "type": "doc",
            "content": [{
                "type": "paragraph",
                "content": [{
                    "type": "image",
                    "attrs": {
                        "src": "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=",
                        "alt": "pixel",
                        "width": 640,
                        "height": 480
                    }
                }]
            }]
        });
        let bytes = export_doc_to_docx(&source, "Dims").expect("export image dims");
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).expect("read docx");
        let mut document = String::new();
        archive
            .by_name("word/document.xml")
            .expect("document part")
            .read_to_string(&mut document)
            .expect("read document");
        // 640px -> 6096000 EMU, 480px -> 4572000 EMU (9525 EMU per px).
        assert!(
            document.contains("cx=\"6096000\"") && document.contains("cy=\"4572000\""),
            "image extents must honor width/height attrs, got: {document}"
        );
    }
}
