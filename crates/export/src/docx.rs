use crate::base64_util::{decode_base64, encode_base64};
use crate::pdf::ExportError;
use docx_rs::*;
use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::Read;
use std::path::Path;

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

#[derive(Clone, Copy)]
struct DocxPageSetup {
    width: u32,
    height: u32,
    orientation: PageOrientationType,
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
    Some(DocxPageSetup {
        width,
        height,
        orientation,
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
        if let Some(href) = hyperlink.or(self.link_href.as_deref()) {
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
}

fn flush_open_list(open_list: &mut Option<OpenList>, paragraphs: &mut Vec<Value>) {
    if let Some(list) = open_list.take() {
        paragraphs.push(json!({
            "type": list.list_type,
            "content": list.items
        }));
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
                return p.add_run(Run::new().add_image(Pic::new_with_dimensions(bytes, 320, 240)));
            }
        }
        return p;
    }

    let Some(text) = child.get("text").and_then(|t| t.as_str()) else {
        return p;
    };

    let mut p = p;
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
            let hyperlink = Hyperlink::new(url, HyperlinkType::External).add_run(run);
            p = p.add_hyperlink(hyperlink);
        } else if let Some((author, date)) = tracked_change_metadata(child, "trackInsert") {
            p = p.add_insert(Insert::new(run).author(author).date(date));
        } else if let Some((author, date)) = tracked_change_metadata(child, "trackDelete") {
            p = p.add_delete(Delete::new().author(author).date(date).add_run(run));
        } else {
            p = p.add_run(run);
        }
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
    comments
        .iter()
        .enumerate()
        .filter_map(|(index, value)| {
            let from = value.get("from").and_then(Value::as_u64)? as usize;
            let to = value.get("to").and_then(Value::as_u64)? as usize;
            if from >= to {
                return None;
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
            Some(ExportCommentAnchor {
                id: index.saturating_add(1),
                from,
                to,
                comment: Comment::new(index.saturating_add(1))
                    .author(author)
                    .date(date)
                    .add_paragraph(Paragraph::new().add_run(Run::new().add_text(text))),
            })
        })
        .collect()
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
    let mut docx = add_list_numbering(Docx::new());
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
    ) {
        let node_type = node.get("type").and_then(|t| t.as_str());

        if node_type == Some("table") {
            let mut rows = Vec::new();
            if let Some(row_nodes) = node.get("content").and_then(|c| c.as_array()) {
                for row in row_nodes {
                    let mut cells = Vec::new();
                    if let Some(cell_nodes) = row.get("content").and_then(|c| c.as_array()) {
                        for cell in cell_nodes {
                            let mut tc = TableCell::new();
                            if let Some(attrs) = cell.get("attrs") {
                                if let Some(colspan) = attrs.get("colspan").and_then(|v| v.as_u64())
                                {
                                    if colspan > 1 {
                                        tc = tc.grid_span(colspan as usize);
                                    }
                                }
                                if let Some(rowspan) = attrs.get("rowspan").and_then(|v| v.as_u64())
                                {
                                    if rowspan > 1 {
                                        tc = tc.vertical_merge(VMergeType::Restart);
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
                            }

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
                            cells.push(tc);
                        }
                    }
                    rows.push(TableRow::new(cells));
                }
            }
            *docx = docx
                .clone()
                .add_table(Table::new(rows).set_borders(TableBorders::new()));
            return;
        }

        if node_type == Some("page_break") {
            *docx = docx
                .clone()
                .add_paragraph(Paragraph::new().add_run(Run::new().add_break(BreakType::Page)));
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
                    );
                }
            }
            return;
        }

        if node_type == Some("ordered_list") {
            let level = list_info.map(|(_, lvl)| lvl + 1).unwrap_or(0);
            if let Some(content) = node.get("content").and_then(|c| c.as_array()) {
                for child in content {
                    build_nodes(
                        child,
                        docx,
                        Some((2, level)),
                        paragraph_layouts,
                        paragraph_index,
                    );
                }
            }
            return;
        }

        if node_type == Some("list_item") {
            if let Some(content) = node.get("content").and_then(|c| c.as_array()) {
                for child in content {
                    build_nodes(child, docx, list_info, paragraph_layouts, paragraph_index);
                }
            }
            return;
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
                    _ => "Heading1",
                });
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
                if let Some(line_height) = attrs.get("lineHeight").and_then(|value| value.as_f64())
                {
                    p = p.line_spacing(
                        LineSpacing::new()
                            .line((line_height * 240.0) as i32)
                            .line_rule(LineSpacingType::Auto),
                    );
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
                build_nodes(child, docx, list_info, paragraph_layouts, paragraph_index);
            }
        }
    }

    build_nodes(
        doc_json,
        &mut docx,
        None,
        &paragraph_layouts,
        &mut paragraph_index,
    );

    docx = apply_export_page_setup(docx, doc_json);

    let mut buf = Vec::new();
    docx.build()
        .pack(std::io::Cursor::new(&mut buf))
        .map_err(|e| ExportError::Docx(format!("{:?}", e)))?;
    Ok(buf)
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
                        let package_path = format!(
                            "word/{}",
                            target.trim_start_matches("../").trim_start_matches('/')
                        );
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
        }
    }

    let mut reader = Reader::from_str(&document);
    reader.config_mut().trim_text(true);
    let mut paragraphs = Vec::new();
    let mut table_rows: Option<Vec<Value>> = None;
    let mut row_cells: Option<Vec<Value>> = None;
    let mut cell_content: Option<Vec<Value>> = None;
    let mut current_content: Vec<Value> = Vec::new();
    let mut current_text = String::new();
    let mut run_state = RunImportState::new();
    let mut paragraph_style: Option<String> = None;
    let mut paragraph_num_id: Option<u32> = None;
    let mut in_text = false;
    let mut pending_image: Option<Value> = None;
    let mut active_hyperlink: Option<String> = None;
    let mut active_tracked_change: Option<TrackedChangeContext> = None;
    let mut document_position = 0usize;
    let mut paragraph_start = 0usize;
    let mut paragraph_text_units = 0usize;
    let mut comment_ranges: HashMap<usize, (usize, usize)> = HashMap::new();
    let mut open_list: Option<OpenList> = None;
    let mut buffer = Vec::new();

    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(event)) => match event.name().as_ref() {
                b"w:tbl" => table_rows = Some(Vec::new()),
                b"w:tr" => row_cells = Some(Vec::new()),
                b"w:tc" => cell_content = Some(Vec::new()),
                b"w:p" => {
                    paragraph_start = document_position;
                    paragraph_text_units = 0;
                    current_content = Vec::new();
                    paragraph_style = None;
                    paragraph_num_id = None;
                }
                b"w:commentRangeStart" => {
                    if let Some(id) = docx_attr(&event, b"id").and_then(|value| value.parse().ok())
                    {
                        eprintln!(
                            "comment range start {id} pos {paragraph_start} {paragraph_text_units}"
                        );
                        let position = paragraph_start
                            .saturating_add(1)
                            .saturating_add(paragraph_text_units);
                        comment_ranges.insert(id, (position, position));
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
                        if attribute.key.as_ref() == b"r:id" {
                            let id = String::from_utf8_lossy(&attribute.value).to_string();
                            active_hyperlink = hyperlink_data.get(&id).cloned();
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
            Ok(Event::Text(event)) if in_text => {
                current_text.push_str(
                    &event
                        .unescape()
                        .map_err(|error| ExportError::Docx(error.to_string()))?,
                );
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
                b"w:r" => {
                    paragraph_text_units =
                        paragraph_text_units.saturating_add(utf16_len(&current_text));
                    if !current_text.is_empty() {
                        let marks = run_state
                            .to_marks(active_hyperlink.as_deref(), active_tracked_change.as_ref());
                        let mut run = json!({ "type": "text", "text": current_text });
                        if !marks.is_empty() {
                            run["marks"] = Value::Array(marks);
                        }
                        current_content.push(run);
                    }
                    current_text.clear();
                    run_state.clear();
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
                        json!({ "type": "heading", "attrs": { "level": level.clamp(1, 3) }, "content": current_content })
                    } else {
                        json!({ "type": "paragraph", "content": current_content })
                    };
                    if let Some(image) = pending_image.take() {
                        if let Some(content) = node.get_mut("content").and_then(Value::as_array_mut)
                        {
                            content.push(image);
                        }
                    }
                    if let Some(content) = cell_content.as_mut() {
                        content.push(node);
                    } else if let Some(num_id) = paragraph_num_id {
                        let list_type = list_type_for_num_id(num_id, &num_formats);
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
                                });
                            }
                        } else {
                            open_list = Some(OpenList {
                                list_type,
                                num_id,
                                items: vec![list_item],
                            });
                        }
                    } else {
                        flush_open_list(&mut open_list, &mut paragraphs);
                        paragraphs.push(node);
                    }
                }
                b"w:tc" => {
                    let cell = json!({
                        "type": "table_cell",
                        "content": cell_content.take().unwrap_or_default()
                    });
                    if let Some(cells) = row_cells.as_mut() {
                        cells.push(cell);
                    }
                }
                b"w:tr" => {
                    let row = json!({
                        "type": "table_row",
                        "content": row_cells.take().unwrap_or_default()
                    });
                    if let Some(rows) = table_rows.as_mut() {
                        rows.push(row);
                    }
                }
                b"w:tbl" => {
                    paragraphs.push(json!({
                        "type": "table",
                        "content": table_rows.take().unwrap_or_default()
                    }));
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
    fn imports_exported_docx_paragraphs_and_marks() {
        let path = std::env::temp_dir().join(format!("redoc-docx-{}.docx", std::process::id()));
        let source = json!({ "type": "doc", "content": [{ "type": "paragraph", "content": [{ "type": "text", "text": "Hello", "marks": [{ "type": "bold" }] }] }] });
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
            .start_file("word/document.xml", zip::write::FileOptions::default())
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
            .start_file("word/document.xml", zip::write::FileOptions::default())
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
                    zip::write::FileOptions::default(),
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
    fn imports_embedded_images_as_data_uris() {
        let path =
            std::env::temp_dir().join(format!("redoc-docx-image-{}.docx", std::process::id()));
        let file = std::fs::File::create(&path).expect("create fixture");
        let mut archive = zip::ZipWriter::new(file);
        let options = zip::write::FileOptions::default();
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
        assert!(
            names.contains(&"word/media/rIdImage1.png".to_string()),
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
}
