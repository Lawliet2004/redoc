use crate::base64_util::{decode_base64, encode_base64};
use crate::pdf::ExportError;
use docx_rs::*;
use quick_xml::events::Event;
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

fn read_docx_xml(archive: &mut zip::ZipArchive<std::fs::File>, name: &str) -> Result<String, ExportError> {
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
        b"w:hyperlink" => "Hyperlinks were not imported.",
        b"w:fldSimple" | b"w:instrText" => "Word fields were skipped.",
        b"w:br" => "Manual line breaks were simplified.",
        b"w:sectPr" => "Section/page layout settings were skipped.",
        b"w:bookmarkStart" | b"w:bookmarkEnd" => "Bookmarks were skipped.",
        b"w:commentRangeStart" | b"w:commentRangeEnd" => "Comments were skipped.",
        _ => return,
    };
    if !warnings.iter().any(|existing| existing == warning) {
        warnings.push(warning.to_string());
    }
}

fn build_run(child: &serde_json::Value, override_font: Option<&str>) -> (Run, Option<String>) {
    let mut link_url = None;
    let mut run = Run::new();

    if let Some(text) = child.get("text").and_then(|t| t.as_str()) {
        run = run.add_text(text);
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
                // Native w:ins/w:del parts are not emitted yet, but retaining
                // review state as visible markup keeps the export auditable.
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
        } else {
            p = p.add_run(run);
        }
    }
    p
}

pub fn export_doc_to_docx(
    doc_json: &serde_json::Value,
    _title: &str,
) -> Result<Vec<u8>, ExportError> {
    let mut docx = add_list_numbering(Docx::new());

    fn build_nodes(node: &serde_json::Value, docx: &mut Docx, list_info: Option<(u32, u32)>) {
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
                                    if let Some(p_content) =
                                        p_node.get("content").and_then(|c| c.as_array())
                                    {
                                        for child in p_content {
                                            cell_p = add_runs_to_paragraph(cell_p, child, None);
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
            *docx = docx.clone().add_table(Table::new(rows).set_borders(TableBorders::new()));
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
            p.property = p.property.shading(Shading::new().fill("F4F4F4"));
            if let Some(content) = node.get("content").and_then(|c| c.as_array()) {
                for child in content {
                    p = add_runs_to_paragraph(p, child, Some("Courier New"));
                }
            }
            *docx = docx.clone().add_paragraph(p);
            return;
        }

        if node_type == Some("bullet_list") {
            let level = list_info.map(|(_, lvl)| lvl + 1).unwrap_or(0);
            if let Some(content) = node.get("content").and_then(|c| c.as_array()) {
                for child in content {
                    build_nodes(child, docx, Some((1, level)));
                }
            }
            return;
        }

        if node_type == Some("ordered_list") {
            let level = list_info.map(|(_, lvl)| lvl + 1).unwrap_or(0);
            if let Some(content) = node.get("content").and_then(|c| c.as_array()) {
                for child in content {
                    build_nodes(child, docx, Some((2, level)));
                }
            }
            return;
        }

        if node_type == Some("list_item") {
            if let Some(content) = node.get("content").and_then(|c| c.as_array()) {
                for child in content {
                    build_nodes(child, docx, list_info);
                }
            }
            return;
        }

        if let Some("paragraph" | "heading" | "blockquote") = node_type {
            let mut p = Paragraph::new();

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
                    p = add_runs_to_paragraph(p, child, None);
                }
            }
            *docx = docx.clone().add_paragraph(p);
            return;
        }

        if let Some(content) = node.get("content").and_then(|c| c.as_array()) {
            for child in content {
                build_nodes(child, docx, list_info);
            }
        }
    }

    build_nodes(doc_json, &mut docx, None);

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

    let mut image_data: HashMap<String, String> = HashMap::new();
    let mut hyperlink_data: HashMap<String, String> = HashMap::new();
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
                            total_media_bytes = total_media_bytes.saturating_add(bytes.len() as u64);
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
    let mut open_list: Option<OpenList> = None;
    let mut buffer = Vec::new();

    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(event)) => match event.name().as_ref() {
                b"w:tbl" => table_rows = Some(Vec::new()),
                b"w:tr" => row_cells = Some(Vec::new()),
                b"w:tc" => cell_content = Some(Vec::new()),
                b"w:p" => {
                    current_content = Vec::new();
                    paragraph_style = None;
                    paragraph_num_id = None;
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
                            paragraph_num_id = String::from_utf8_lossy(&attribute.value)
                                .parse()
                                .ok();
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

    let raw_document = json!({ "type": "doc", "content": paragraphs });
    let document = redoc_doc_engine::prune_doc(&raw_document);

    Ok(DocxImportResult {
        document,
        warnings,
    })
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
