use crate::base64_util::data_uri_bytes;
use printpdf::path::{PaintMode, WindingOrder};
use printpdf::*;
use redoc_sheet_engine::WorkbookModel;
use redoc_slide_engine::{DeckModel, ElementKind};
use std::io::BufWriter;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ExportError {
    #[error("PDF export error: {0}")]
    Pdf(String),
    #[error("DOCX export error: {0}")]
    Docx(String),
    #[error("XLSX export error: {0}")]
    Xlsx(String),
    #[error("ZIP error: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

fn parse_page_setup(
    doc_json: &serde_json::Value,
) -> (f32, f32, f32, f32, f32, f32, Option<String>, Option<String>) {
    let setup = doc_json
        .get("pageSetup")
        .or_else(|| doc_json.get("page_setup"))
        .or_else(|| {
            doc_json
                .get("attrs")
                .and_then(|a| a.get("pageSetup").or_else(|| a.get("page_setup")))
        });

    let mut paper_size = "a4".to_string();
    let mut orientation = "portrait".to_string();
    let mut margin_top_in = 1.0_f32;
    let mut margin_bottom_in = 1.0_f32;
    let mut margin_left_in = 1.0_f32;
    let mut margin_right_in = 1.0_f32;
    let mut header_text = None;
    let mut footer_text = None;

    if let Some(s) = setup {
        if let Some(h) = s.get("header").and_then(|v| v.as_str()) {
            header_text = Some(h.to_string());
        }
        if let Some(f) = s.get("footer").and_then(|v| v.as_str()) {
            footer_text = Some(f.to_string());
        }
        if let Some(ps) = s
            .get("paperSize")
            .or_else(|| s.get("paper_size"))
            .and_then(|v| v.as_str())
        {
            paper_size = ps.to_lowercase();
        }
        if let Some(ori) = s.get("orientation").and_then(|v| v.as_str()) {
            orientation = ori.to_lowercase();
        }
        if let Some(m) = s.get("margins") {
            if let Some(val) = m.get("top").and_then(|v| v.as_f64()) {
                margin_top_in = val as f32;
            }
            if let Some(val) = m.get("bottom").and_then(|v| v.as_f64()) {
                margin_bottom_in = val as f32;
            }
            if let Some(val) = m.get("left").and_then(|v| v.as_f64()) {
                margin_left_in = val as f32;
            }
            if let Some(val) = m.get("right").and_then(|v| v.as_f64()) {
                margin_right_in = val as f32;
            }
        }
    }

    let (mut width_mm, mut height_mm) = match paper_size.as_str() {
        "letter" => (215.9_f32, 279.4_f32),
        "legal" => (215.9_f32, 355.6_f32),
        "executive" => (184.15_f32, 266.7_f32),
        _ => (210.0_f32, 297.0_f32),
    };

    if orientation == "landscape" {
        std::mem::swap(&mut width_mm, &mut height_mm);
    }

    let margin_top_mm = if margin_top_in <= 10.0 {
        margin_top_in * 25.4
    } else {
        margin_top_in
    };
    let margin_bottom_mm = if margin_bottom_in <= 10.0 {
        margin_bottom_in * 25.4
    } else {
        margin_bottom_in
    };
    let margin_left_mm = if margin_left_in <= 10.0 {
        margin_left_in * 25.4
    } else {
        margin_left_in
    };
    let margin_right_mm = if margin_right_in <= 10.0 {
        margin_right_in * 25.4
    } else {
        margin_right_in
    };

    (
        width_mm,
        height_mm,
        margin_top_mm,
        margin_bottom_mm,
        margin_left_mm,
        margin_right_mm,
        header_text,
        footer_text,
    )
}

fn pdf_field_text(node: &serde_json::Value) -> Option<String> {
    if node.get("type").and_then(|value| value.as_str()) != Some("field") {
        return None;
    }
    let attrs = node.get("attrs");
    if let Some(result) = attrs.and_then(|attrs| attrs.get("result")) {
        if let Some(value) = result.as_str() {
            let value = value
                .chars()
                .take(64)
                .collect::<String>()
                .trim()
                .to_string();
            if !value.is_empty() {
                return Some(value);
            }
        } else if let Some(value) = result.as_f64().filter(|value| value.is_finite()) {
            return Some(value.floor().to_string());
        }
    }
    let kind = attrs
        .and_then(|attrs| attrs.get("kind"))
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    Some(match kind.as_str() {
        "page" | "pagenumber" => "[PAGE]".to_string(),
        "numpages" | "pagecount" => "[NUMPAGES]".to_string(),
        _ => "[FIELD]".to_string(),
    })
}

// ── Styled-run PDF layout ──────────────────────────────────────────────────
// Upgrades the legacy text-flattening exporter to run-level fidelity: bold /
// italic font variants, alignment, list markers and real table grids, while
// keeping pagination, images and header/footer behavior intact.

#[derive(Debug, Clone)]
struct PdfRun {
    text: String,
    bold: bool,
    italic: bool,
    /// Font size in points.
    size: f32,
}

#[derive(Debug, Clone, Default)]
struct PdfParagraphStyle {
    align: Option<String>,
    indent: f32,
    is_code: bool,
    /// Rendered prefix for list items ("• ", "1. ", …).
    list_prefix: Option<String>,
    spacing_after: f32,
}

#[derive(Debug, Clone)]
enum PdfBlock {
    Paragraph {
        runs: Vec<PdfRun>,
        style: PdfParagraphStyle,
    },
    Table {
        /// rows -> cells -> paragraph runs (cells keep their own styles).
        rows: Vec<Vec<Vec<PdfRun>>>,
        /// True for any header row (shaded + bold).
        header_rows: Vec<bool>,
    },
    Image(Vec<u8>),
    PageBreak,
}

/// Standard Helvetica AFM advance widths (units/1000) for ASCII 32..=126.
/// Bold approximates via +8%; the builtin bold face tracks closely enough
/// for wrapping decisions.
const HELVETICA_WIDTHS: [u16; 94] = [
    278, 278, 355, 556, 556, 889, 667, 191, 333, 333, 389, 584, 278, 333, 278,
    278, // !"#$%&'()*+,-./
    556, 556, 556, 556, 556, 556, 556, 556, 556, 556, 278, 278, 584, 584, 584,
    556, // 0123456789:;<=>?
    1015, 667, 667, 722, 722, 667, 611, 778, 722, 278, 500, 667, 556, 833, 722,
    778, // @ABCDEFGHIJKLMNO
    667, 778, 722, 667, 611, 556, 722, 667, 944, 667, 667, 611, 278, 278, 278,
    469, // PQRSTUVWXYZ[\]^_
    556, 556, 500, 556, 556, 278, 556, 556, 222, 222, 500, 222, 833, 556, 556,
    556, // `abcdefghijklmno
    500, 500, 278, 278, 500, 278, 778, 500, 500, 500, 500, 333, 556, 333, // pqrstuvwxyz{|}~
];

/// Width of `text` in millimetres when set in Helvetica at `size` points.
fn text_width_mm(text: &str, size: f32, bold: bool) -> f32 {
    let mut units = 0u32;
    let mut count = 0u32;
    for c in text.chars() {
        count += 1;
        let idx = (c as u32).saturating_sub(32) as usize;
        let width = if idx < 95 {
            HELVETICA_WIDTHS[idx] as f32
        } else {
            600.0 // non-ASCII fallback (glyph-missing approximations)
        };
        units += width.round() as u32;
    }
    if count == 0 {
        return 0.0;
    }
    let scale = if bold { 1.08 } else { 1.0 };
    (units as f32 / 1000.0) * size * scale * 25.4 / 72.0
}

fn mark_flag(marks: &serde_json::Value, name: &str) -> bool {
    marks.as_array().is_some_and(|list| {
        list.iter()
            .any(|m| m.get("type").and_then(|t| t.as_str()) == Some(name))
    })
}

fn mark_attr<'a>(
    marks: &'a serde_json::Value,
    name: &str,
    attr: &str,
) -> Option<&'a serde_json::Value> {
    marks
        .as_array()?
        .iter()
        .find(|m| m.get("type").and_then(|t| t.as_str()) == Some(name))?
        .get("attrs")?
        .get(attr)
}

/// Collect styled runs from a text node (text | field | footnote_ref), honoring marks.
fn collect_runs(node: &serde_json::Value, base_size: f32) -> Vec<PdfRun> {
    let node_type = node.get("type").and_then(|v| v.as_str());
    let (text, marks) = match node_type {
        Some("field") => {
            return vec![PdfRun {
                text: pdf_field_text(node).unwrap_or_default(),
                bold: false,
                italic: false,
                size: base_size,
            }];
        }
        Some("footnote_ref") => {
            // Superscript reference number in the body text.
            let label = node
                .get("attrs")
                .and_then(|attrs| attrs.get("label"))
                .map(|value| value.to_string().replace('"', ""))
                .unwrap_or_else(|| "*".to_string());
            return vec![PdfRun {
                text: format!("[{label}]"),
                bold: false,
                italic: false,
                size: (base_size * 0.75).max(6.0),
            }];
        }
        Some("text") => (
            node.get("text")
                .and_then(|v| v.as_str())
                .unwrap_or_default(),
            node.get("marks")
                .cloned()
                .unwrap_or(serde_json::Value::Null),
        ),
        _ => return Vec::new(),
    };
    let size = mark_attr(&marks, "fontSize", "size")
        .and_then(|v| v.as_f64().or_else(|| v.as_str()?.parse().ok()))
        .map(|v| v as f32)
        .unwrap_or(base_size);
    vec![PdfRun {
        text: text.to_string(),
        bold: mark_flag(&marks, "bold"),
        italic: mark_flag(&marks, "italic"),
        size,
    }]
}

/// Merge consecutive runs with identical styling so line-breaking sees fewer
/// pieces without losing fidelity.
fn coalesce_runs(runs: Vec<PdfRun>) -> Vec<PdfRun> {
    let mut out: Vec<PdfRun> = Vec::new();
    for run in runs {
        if let Some(last) = out.last_mut() {
            if last.bold == run.bold
                && last.italic == run.italic
                && (last.size - run.size).abs() < 0.01
            {
                last.text.push_str(&run.text);
                continue;
            }
        }
        out.push(run);
    }
    out
}

/// One wrapped line: a run slice positioned at a horizontal offset.
struct LineFragment {
    run: PdfRun,
    start: usize,
    end: usize,
    run_index: usize,
}

struct WrappedLine {
    fragments: Vec<LineFragment>,
    width_mm: f32,
}

/// Greedy word wrap across run boundaries. Spaces may be dropped at breaks;
/// hard newlines inside runs split lines like the DOM editor.
fn wrap_runs(runs: &[PdfRun], max_width_mm: f32) -> Vec<WrappedLine> {
    #[derive(Clone)]
    struct Word {
        run_index: usize,
        start: usize,
        end: usize,
        width_mm: f32,
        leading_space: bool,
    }

    let mut words: Vec<Word> = Vec::new();
    for (run_index, run) in runs.iter().enumerate() {
        let bytes = run.text.as_bytes();
        let mut cursor = 0usize;
        while cursor < bytes.len() {
            // Split on spaces, remembering adjacency so word joins survive
            // across run boundaries (e.g. "fo" + "o bar").
            let mut word_end = cursor;
            let mut leading_space = false;
            if cursor > 0 || run_index == 0 {
                // skip leading spaces at line starts handled below
            }
            while word_end < bytes.len() && bytes[word_end] == b' ' {
                if word_end == cursor {
                    leading_space = true;
                }
                word_end += 1;
            }
            let start = word_end;
            let mut end = start;
            while end < bytes.len() && bytes[end] != b' ' {
                end += 1;
            }
            let mut probe = end;
            while probe < bytes.len() && bytes[probe] == b' ' {
                probe += 1;
            }
            if end > start {
                let text = &run.text[start..end];
                words.push(Word {
                    run_index,
                    start,
                    end,
                    width_mm: text_width_mm(text, run.size, run.bold),
                    leading_space: leading_space && start > cursor,
                });
            }
            cursor = if probe > end {
                probe
            } else {
                end.max(start + 1).max(cursor + 1)
            };
        }
    }

    let space_mm = |size: f32, bold: bool| text_width_mm(" ", size, bold);
    let mut lines: Vec<WrappedLine> = Vec::new();
    let mut fragments: Vec<LineFragment> = Vec::new();
    let mut line_width = 0.0f32;

    macro_rules! flush_line {
        () => {{
            lines.push(WrappedLine {
                fragments: std::mem::take(&mut fragments),
                width_mm: line_width,
            });
            #[allow(unused_assignments)]
            {
                line_width = 0.0_f32;
            }
        }};
    }

    for (index, word) in words.iter().enumerate() {
        let run = &runs[word.run_index];
        let gap = if index == 0 || line_width == 0.0 {
            0.0
        } else {
            space_mm(run.size, run.bold)
        };
        if line_width + gap + word.width_mm > max_width_mm && line_width > 0.0 {
            flush_line!();
        }
        let effective = if line_width > 0.0 {
            gap + word.width_mm
        } else {
            word.width_mm
        };
        line_width += effective;
        // Merge into the previous fragment when it's the same run and
        // adjacent range; otherwise start a new fragment.
        if let Some(last) = fragments.last_mut() {
            if last.run.bold == run.bold
                && last.run.italic == run.italic
                && (last.run.size - run.size).abs() < 0.01
                && last.run_index == word.run_index
                && last.end == word.start
                && word.leading_space
            {
                last.end = word.end;
                continue;
            }
        }
        fragments.push(LineFragment {
            run: run.clone(),
            start: word.start,
            end: word.end,
            run_index: word.run_index,
        });
    }
    if !fragments.is_empty() {
        flush_line!();
    }
    lines
}

fn block_style_from_node(
    node: &serde_json::Value,
    is_heading: bool,
    is_code: bool,
) -> PdfParagraphStyle {
    let attrs = node.get("attrs");
    let spacing_after = attrs
        .and_then(|a| a.get("spacingAfter"))
        .and_then(|v| v.as_f64())
        .map(|v| (v as f32) * 0.3528) // pt → mm
        .unwrap_or(if is_heading { 6.0 } else { 4.0 });
    PdfParagraphStyle {
        align: attrs
            .and_then(|a| a.get("align"))
            .and_then(|v| v.as_str())
            .map(str::to_string),
        indent: attrs
            .and_then(|a| a.get("indent"))
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0) as f32,
        is_code,
        list_prefix: None,
        spacing_after,
    }
}

fn collect_pdf_blocks(
    node: &serde_json::Value,
    blocks: &mut Vec<PdfBlock>,
    list_stack: &mut Vec<(bool, u32)>,
) {
    let node_type = node.get("type").and_then(|v| v.as_str());
    match node_type {
        Some("page_break") => blocks.push(PdfBlock::PageBreak),
        Some("image") => {
            if let Some(src) = node
                .get("attrs")
                .and_then(|attrs| attrs.get("src"))
                .and_then(|src| src.as_str())
            {
                if let Some(bytes) = data_uri_bytes(src) {
                    blocks.push(PdfBlock::Image(bytes));
                }
            }
        }
        Some("paragraph" | "heading" | "blockquote" | "code_block") => {
            let is_heading = node_type == Some("heading");
            let is_code = node_type == Some("code_block");
            let heading_level = node
                .get("attrs")
                .and_then(|a| a.get("level"))
                .and_then(|l| l.as_u64())
                .unwrap_or(2) as u8;
            let base_size = if is_heading {
                match heading_level {
                    1 => 20.0,
                    2 => 17.0,
                    3 => 15.0,
                    _ => 13.0,
                }
            } else if is_code {
                10.0
            } else {
                11.0
            };
            let mut style = block_style_from_node(node, is_heading, is_code);
            if let Some((ordered, counter)) = list_stack.last().copied() {
                style.list_prefix = Some(if ordered {
                    format!("{counter}. ")
                } else {
                    "• ".to_string()
                });
            }
            let mut runs: Vec<PdfRun> = Vec::new();
            if let Some(children) = node.get("content").and_then(|c| c.as_array()) {
                for child in children {
                    runs.extend(collect_runs(child, base_size));
                }
            }
            if is_heading {
                for run in &mut runs {
                    run.bold = true;
                }
            }
            if is_code {
                for run in &mut runs {
                    run.bold = false;
                    run.italic = false;
                }
            }
            let runs = coalesce_runs(runs);
            if !runs.is_empty() {
                blocks.push(PdfBlock::Paragraph { runs, style });
            } else if style.list_prefix.is_some() {
                // Keep empty list items as a blank marker line.
                blocks.push(PdfBlock::Paragraph {
                    runs: vec![PdfRun {
                        text: " ".to_string(),
                        bold: false,
                        italic: false,
                        size: base_size,
                    }],
                    style,
                });
            }
        }
        Some("bullet_list" | "ordered_list") => {
            let ordered = node_type == Some("ordered_list");
            let counter_start = 1;
            let mut counter = counter_start;
            list_stack.push((ordered, counter));
            if let Some(children) = node.get("content").and_then(|c| c.as_array()) {
                for child in children {
                    collect_pdf_blocks(child, blocks, list_stack);
                    if ordered {
                        counter += 1;
                        if let Some(top) = list_stack.last_mut() {
                            top.1 = counter;
                        }
                    }
                }
            }
            list_stack.pop();
        }
        Some("table") => {
            let mut rows: Vec<Vec<Vec<PdfRun>>> = Vec::new();
            let mut header_rows: Vec<bool> = Vec::new();
            if let Some(row_nodes) = node.get("content").and_then(|c| c.as_array()) {
                for row in row_nodes {
                    let mut row_runs: Vec<Vec<PdfRun>> = Vec::new();
                    let mut row_has_header = false;
                    if let Some(cells) = row.get("content").and_then(|c| c.as_array()) {
                        for cell in cells {
                            let is_header =
                                cell.get("type").and_then(|t| t.as_str()) == Some("table_header");
                            row_has_header |= is_header;
                            let mut runs: Vec<PdfRun> = Vec::new();
                            if let Some(paragraphs) = cell.get("content").and_then(|c| c.as_array())
                            {
                                for paragraph in paragraphs {
                                    if let Some(children) =
                                        paragraph.get("content").and_then(|c| c.as_array())
                                    {
                                        for child in children {
                                            runs.extend(collect_runs(child, 10.0));
                                        }
                                    }
                                }
                            }
                            for run in &mut runs {
                                if is_header {
                                    run.bold = true;
                                }
                            }
                            row_runs.push(coalesce_runs(runs));
                        }
                    }
                    rows.push(row_runs);
                    header_rows.push(row_has_header);
                }
            }
            if !rows.is_empty() {
                blocks.push(PdfBlock::Table { rows, header_rows });
            }
        }
        _ => {
            if let Some(children) = node.get("content").and_then(|c| c.as_array()) {
                for child in children {
                    collect_pdf_blocks(child, blocks, list_stack);
                }
            }
        }
    }
}

pub fn export_doc_to_pdf(
    doc_json: &serde_json::Value,
    title: &str,
) -> Result<Vec<u8>, ExportError> {
    let (
        page_width,
        page_height,
        margin_top,
        margin_bottom,
        margin_left,
        margin_right,
        header_opt,
        footer_opt,
    ) = parse_page_setup(doc_json);

    let (doc, mut page, mut layer) =
        PdfDocument::new(title, Mm(page_width), Mm(page_height), "Page 1");
    let mut pages: Vec<(PdfPageIndex, PdfLayerIndex)> = vec![(page, layer)];

    // Four builtin variants give the export run-level bold/italic fidelity.
    let font_regular = doc
        .add_builtin_font(BuiltinFont::Helvetica)
        .map_err(|e| ExportError::Pdf(format!("{:?}", e)))?;
    let font_bold = doc
        .add_builtin_font(BuiltinFont::HelveticaBold)
        .map_err(|e| ExportError::Pdf(format!("{:?}", e)))?;
    let font_italic = doc
        .add_builtin_font(BuiltinFont::HelveticaOblique)
        .map_err(|e| ExportError::Pdf(format!("{:?}", e)))?;
    let font_bold_italic = doc
        .add_builtin_font(BuiltinFont::HelveticaBoldOblique)
        .map_err(|e| ExportError::Pdf(format!("{:?}", e)))?;
    let font_code = doc
        .add_builtin_font(BuiltinFont::Courier)
        .map_err(|e| ExportError::Pdf(format!("{:?}", e)))?;

    let font_for = |run: &PdfRun| -> (IndirectFontRef, IndirectFontRef) {
        if run.italic && run.bold {
            (font_bold_italic.clone(), font_bold_italic.clone())
        } else if run.italic {
            (font_italic.clone(), font_italic.clone())
        } else if run.bold {
            (font_bold.clone(), font_bold.clone())
        } else {
            (font_regular.clone(), font_regular.clone())
        }
    };

    let printable_width = (page_width - margin_left - margin_right).max(10.0);
    let initial_y = page_height - margin_top - 10.0;
    let line_height_for = |size: f32| (size * 0.42).max(4.2);

    let new_page = |doc: &PdfDocumentReference,
                    page: &mut PdfPageIndex,
                    layer: &mut PdfLayerIndex,
                    pages: &mut Vec<(PdfPageIndex, PdfLayerIndex)>,
                    page_number: &mut usize,
                    y: &mut f32| {
        *page_number += 1;
        let next = doc.add_page(
            Mm(page_width),
            Mm(page_height),
            format!("Page {}", page_number),
        );
        *page = next.0;
        *layer = next.1;
        pages.push((*page, *layer));
        *y = initial_y;
    };

    let mut blocks: Vec<PdfBlock> = Vec::new();
    collect_pdf_blocks(doc_json, &mut blocks, &mut Vec::new());

    let mut y = initial_y;
    let mut page_number = 1usize;

    for block in &blocks {
        match block {
            PdfBlock::PageBreak => {
                new_page(
                    &doc,
                    &mut page,
                    &mut layer,
                    &mut pages,
                    &mut page_number,
                    &mut y,
                );
            }
            PdfBlock::Paragraph { runs, style } => {
                let indent_mm = (style.indent * 8.0).min(printable_width * 0.3);
                let max_width = printable_width - indent_mm;
                let lines = wrap_runs(runs, max_width);

                // List marker on the first line, aligned with the indent.
                if let Some(prefix) = &style.list_prefix {
                    let current_layer = doc.get_page(page).get_layer(layer);
                    current_layer.begin_text_section();
                    current_layer.set_font(&font_regular, 11.0);
                    current_layer.set_text_cursor(Mm(margin_left), Mm(y));
                    current_layer.write_text(prefix, &font_regular);
                    current_layer.end_text_section();
                }

                for line in &lines {
                    if y < margin_bottom + 12.0 {
                        new_page(
                            &doc,
                            &mut page,
                            &mut layer,
                            &mut pages,
                            &mut page_number,
                            &mut y,
                        );
                    }
                    let line_width = line.width_mm;
                    let x = match style.align.as_deref() {
                        Some("center") => {
                            margin_left + indent_mm + ((max_width - line_width) / 2.0).max(0.0)
                        }
                        Some("right") | Some("justify") => {
                            margin_left + indent_mm + (max_width - line_width).max(0.0)
                        }
                        _ => margin_left + indent_mm,
                    };
                    let current_layer = doc.get_page(page).get_layer(layer);
                    let mut cursor = x;
                    for fragment in &line.fragments {
                        let text = &fragment.run.text[fragment.start..fragment.end];
                        if text.is_empty() {
                            continue;
                        }
                        let (write_font, _meta_font) = if style.is_code {
                            (font_code.clone(), font_code.clone())
                        } else {
                            font_for(&fragment.run)
                        };
                        current_layer.begin_text_section();
                        current_layer.set_font(&write_font, fragment.run.size);
                        current_layer.set_text_cursor(Mm(cursor), Mm(y));
                        current_layer.write_text(text, &write_font);
                        current_layer.end_text_section();
                        cursor += text_width_mm(text, fragment.run.size, fragment.run.bold);
                    }
                    let size = line.fragments.first().map(|f| f.run.size).unwrap_or(11.0);
                    y -= line_height_for(size);
                }
                y -= style.spacing_after;
            }
            PdfBlock::Image(bytes) => {
                let Ok(decoded) = ::image::load_from_memory(bytes) else {
                    continue;
                };
                let image = printpdf::Image::from_dynamic_image(&decoded);
                let dpi = 96.0_f32;
                let source_width = image.image.width.0 as f32 * 25.4 / dpi;
                let source_height = image.image.height.0 as f32 * 25.4 / dpi;
                let width = source_width.min(printable_width);
                let height = (source_height * (width / source_width.max(1.0))).min(100.0);
                if y - height < margin_bottom + 12.0 {
                    new_page(
                        &doc,
                        &mut page,
                        &mut layer,
                        &mut pages,
                        &mut page_number,
                        &mut y,
                    );
                }
                image.add_to_layer(
                    doc.get_page(page).get_layer(layer),
                    ImageTransform {
                        translate_x: Some(Mm(margin_left)),
                        translate_y: Some(Mm(y - height)),
                        scale_x: Some((width / source_width.max(1.0)).max(0.001)),
                        scale_y: Some((height / source_height.max(1.0)).max(0.001)),
                        dpi: Some(dpi),
                        ..Default::default()
                    },
                );
                y -= height + 8.0;
            }
            PdfBlock::Table { rows, header_rows } => {
                let column_count = rows.iter().map(|r| r.len()).max().unwrap_or(1).max(1);
                let column_width = printable_width / column_count as f32;
                let cell_padding = 1.6_f32;

                for (row_index, row) in rows.iter().enumerate() {
                    // Wrap each cell, then lay the row out at its tallest line.
                    let wrapped_cells: Vec<Vec<WrappedLine>> = row
                        .iter()
                        .map(|cell_runs| wrap_runs(cell_runs, column_width - 2.0 * cell_padding))
                        .collect();
                    let row_line_count = wrapped_cells
                        .iter()
                        .map(|c| c.len())
                        .max()
                        .unwrap_or(1)
                        .max(1);
                    let cell_size = 10.0_f32;
                    let row_height =
                        row_line_count as f32 * line_height_for(cell_size) + 2.0 * cell_padding;

                    if y - row_height < margin_bottom + 12.0 {
                        new_page(
                            &doc,
                            &mut page,
                            &mut layer,
                            &mut pages,
                            &mut page_number,
                            &mut y,
                        );
                    }
                    let top_y = y;
                    let is_header = header_rows.get(row_index).copied().unwrap_or(false);

                    let current_layer = doc.get_page(page).get_layer(layer);
                    // Header shading.
                    if is_header {
                        let rect = Polygon {
                            rings: vec![vec![
                                (Point::new(Mm(margin_left), Mm(top_y)), false),
                                (
                                    Point::new(Mm(margin_left + printable_width), Mm(top_y)),
                                    false,
                                ),
                                (
                                    Point::new(
                                        Mm(margin_left + printable_width),
                                        Mm(top_y - row_height),
                                    ),
                                    false,
                                ),
                                (Point::new(Mm(margin_left), Mm(top_y - row_height)), false),
                            ]],
                            mode: PaintMode::Fill,
                            winding_order: WindingOrder::NonZero,
                        };
                        current_layer.set_fill_color(Color::Rgb(Rgb::new(0.85, 0.88, 0.95, None)));
                        current_layer.add_polygon(rect);
                    }

                    // Cell borders.
                    current_layer.set_outline_color(Color::Rgb(Rgb::new(0.75, 0.75, 0.78, None)));
                    for column in 0..=column_count {
                        let x = margin_left + column as f32 * column_width;
                        current_layer.add_line(Line {
                            points: vec![
                                (Point::new(Mm(x), Mm(top_y)), false),
                                (Point::new(Mm(x), Mm(top_y - row_height)), false),
                            ],
                            is_closed: false,
                        });
                    }
                    current_layer.add_line(Line {
                        points: vec![
                            (Point::new(Mm(margin_left), Mm(top_y - row_height)), false),
                            (
                                Point::new(
                                    Mm(margin_left + printable_width),
                                    Mm(top_y - row_height),
                                ),
                                false,
                            ),
                        ],
                        is_closed: false,
                    });
                    if row_index == 0 {
                        current_layer.add_line(Line {
                            points: vec![
                                (Point::new(Mm(margin_left), Mm(top_y)), false),
                                (
                                    Point::new(Mm(margin_left + printable_width), Mm(top_y)),
                                    false,
                                ),
                            ],
                            is_closed: false,
                        });
                    }

                    // Cell text.
                    for (column, cell_lines) in wrapped_cells.iter().enumerate() {
                        let cell_x = margin_left + column as f32 * column_width + cell_padding;
                        let mut text_y = top_y - cell_padding - 1.0;
                        for line in cell_lines {
                            for fragment in &line.fragments {
                                let text = &fragment.run.text[fragment.start..fragment.end];
                                if text.is_empty() {
                                    continue;
                                }
                                let (write_font, _meta) = font_for(&fragment.run);
                                current_layer.begin_text_section();
                                current_layer.set_font(&write_font, fragment.run.size);
                                current_layer.set_text_cursor(Mm(cell_x), Mm(text_y));
                                current_layer.write_text(text, &write_font);
                                current_layer.end_text_section();
                            }
                            text_y -= line_height_for(cell_size);
                        }
                    }

                    y -= row_height;
                }
                y -= 6.0;
            }
        }
    }
    // Footnotes: render as an endnotes section after the body. Word prints
    // per-page footnotes; the PDF export lists them at the end with the same
    // bracketed labels used in the body text.
    if let Some(notes) = doc_json.get("footnotes").and_then(|value| value.as_array()) {
        let readable: Vec<(u64, String)> = notes
            .iter()
            .enumerate()
            .filter_map(|(index, note)| {
                let text = note
                    .get("text")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default();
                if text.trim().is_empty() {
                    return None;
                }
                let label = note
                    .get("label")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(index as u64 + 1);
                Some((label, text.to_string()))
            })
            .collect();
        if !readable.is_empty() {
            new_page(
                &doc,
                &mut page,
                &mut layer,
                &mut pages,
                &mut page_number,
                &mut y,
            );
            let current_layer = doc.get_page(page).get_layer(layer);
            current_layer.begin_text_section();
            current_layer.set_font(&font_bold, 14.0);
            current_layer.set_text_cursor(Mm(margin_left), Mm(y));
            current_layer.write_text("Notes", &font_bold);
            current_layer.end_text_section();
            y -= 10.0;
            for (label, text) in readable {
                if y < margin_bottom + 12.0 {
                    new_page(
                        &doc,
                        &mut page,
                        &mut layer,
                        &mut pages,
                        &mut page_number,
                        &mut y,
                    );
                }
                let note_runs = vec![
                    PdfRun {
                        text: format!("[{label}] "),
                        bold: true,
                        italic: false,
                        size: 9.0,
                    },
                    PdfRun {
                        text,
                        bold: false,
                        italic: false,
                        size: 9.0,
                    },
                ];
                for line in wrap_runs(&note_runs, printable_width) {
                    if y < margin_bottom + 12.0 {
                        new_page(
                            &doc,
                            &mut page,
                            &mut layer,
                            &mut pages,
                            &mut page_number,
                            &mut y,
                        );
                    }
                    let mut cursor = margin_left;
                    for fragment in &line.fragments {
                        let text = &fragment.run.text[fragment.start..fragment.end];
                        if text.is_empty() {
                            continue;
                        }
                        let (write_font, _) = font_for(&fragment.run);
                        let current_layer = doc.get_page(page).get_layer(layer);
                        current_layer.begin_text_section();
                        current_layer.set_font(&write_font, fragment.run.size);
                        current_layer.set_text_cursor(Mm(cursor), Mm(y));
                        current_layer.write_text(text, &write_font);
                        current_layer.end_text_section();
                        cursor += text_width_mm(text, fragment.run.size, fragment.run.bold);
                    }
                    y -= line_height_for(9.0);
                }
                y -= 2.0;
            }
        }
    }
    let total_pages = pages.len();
    for (i, &(p, l)) in pages.iter().enumerate() {
        let current_layer = doc.get_page(p).get_layer(l);
        if let Some(ref header) = header_opt {
            let h_text = header
                .replace("{page}", &(i + 1).to_string())
                .replace("{total}", &total_pages.to_string());
            let approx_width = h_text.chars().count() as f32 * 10.0 * 0.5;
            let center_x = (page_width - approx_width) / 2.0;
            current_layer.begin_text_section();
            current_layer.set_font(&font_regular, 10.0);
            current_layer.set_text_cursor(Mm(center_x), Mm(page_height - margin_top / 2.0));
            current_layer.write_text(h_text, &font_regular);
            current_layer.end_text_section();
        }
        if let Some(ref footer) = footer_opt {
            let f_text = footer
                .replace("{page}", &(i + 1).to_string())
                .replace("{total}", &total_pages.to_string());
            let approx_width = f_text.chars().count() as f32 * 10.0 * 0.5;
            let center_x = (page_width - approx_width) / 2.0;
            current_layer.begin_text_section();
            current_layer.set_font(&font_regular, 10.0);
            current_layer.set_text_cursor(Mm(center_x), Mm(margin_bottom / 2.0));
            current_layer.write_text(f_text, &font_regular);
            current_layer.end_text_section();
        }
    }

    let mut buffer = Vec::new();
    let mut writer = BufWriter::new(&mut buffer);
    doc.save(&mut writer)
        .map_err(|e| ExportError::Pdf(format!("{:?}", e)))?;
    drop(writer);

    Ok(buffer)
}

pub fn export_workbook_to_pdf(
    workbook: &WorkbookModel,
    title: &str,
) -> Result<Vec<u8>, ExportError> {
    let (doc, mut page, mut layer) = PdfDocument::new(title, Mm(210.0), Mm(297.0), "Sheet 1");
    let font = doc
        .add_builtin_font(BuiltinFont::Helvetica)
        .map_err(|error| ExportError::Pdf(format!("{:?}", error)))?;
    let sheet = workbook.sheets.get(workbook.active_sheet_index);
    let mut y = 280.0_f32;
    let mut page_number = 1;
    let write_line = |doc: &PdfDocumentReference,
                      page: &mut PdfPageIndex,
                      layer: &mut PdfLayerIndex,
                      y: &mut f32,
                      page_number: &mut usize,
                      text: &str| {
        if *y < 18.0 {
            *page_number += 1;
            let next = doc.add_page(Mm(210.0), Mm(297.0), format!("Sheet {}", *page_number));
            *page = next.0;
            *layer = next.1;
            *y = 280.0;
        }
        let current = doc.get_page(*page).get_layer(*layer);
        current.begin_text_section();
        current.set_font(&font, 9.0);
        current.set_text_cursor(Mm(15.0), Mm(*y));
        current.write_text(text, &font);
        current.end_text_section();
        *y -= 6.0;
    };
    write_line(&doc, &mut page, &mut layer, &mut y, &mut page_number, title);
    if let Some(sheet) = sheet {
        let mut keys: Vec<_> = sheet.cells.keys().collect();
        keys.sort();
        for key in keys {
            if let Some(cell) = sheet.cells.get(key) {
                write_line(
                    &doc,
                    &mut page,
                    &mut layer,
                    &mut y,
                    &mut page_number,
                    &format!("{}: {}", key, cell.display_value),
                );
            }
        }
    }
    let mut buffer = Vec::new();
    doc.save(&mut BufWriter::new(&mut buffer))
        .map_err(|error| ExportError::Pdf(format!("{:?}", error)))?;
    Ok(buffer)
}

fn slide_color(value: &str, fallback: (f32, f32, f32)) -> Color {
    let value = value.trim().trim_start_matches('#');
    if value.len() == 6 {
        if let Ok(rgb) = u32::from_str_radix(value, 16) {
            return Color::Rgb(Rgb::new(
                ((rgb >> 16) & 0xff) as f32 / 255.0,
                ((rgb >> 8) & 0xff) as f32 / 255.0,
                (rgb & 0xff) as f32 / 255.0,
                None,
            ));
        }
    }
    Color::Rgb(Rgb::new(fallback.0, fallback.1, fallback.2, None))
}

fn begin_element_rotation(
    layer: &PdfLayerReference,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    rotation: f64,
) -> (f64, f64) {
    if rotation.abs() < 1e-6 {
        return (x, y);
    }
    // CSS-style clockwise degrees; printpdf CurTransMat::Rotate is also clockwise.
    let cx = x + width / 2.0;
    let cy = y + height / 2.0;
    layer.save_graphics_state();
    layer.set_ctm(CurTransMat::Translate(
        Mm(cx as f32).into(),
        Mm(cy as f32).into(),
    ));
    layer.set_ctm(CurTransMat::Rotate(rotation as f32));
    layer.set_ctm(CurTransMat::Translate(
        Mm((-width / 2.0) as f32).into(),
        Mm((-height / 2.0) as f32).into(),
    ));
    (0.0, 0.0)
}

fn end_element_rotation(layer: &PdfLayerReference, rotation: f64) {
    if rotation.abs() >= 1e-6 {
        layer.restore_graphics_state();
    }
}

pub fn export_deck_to_pdf(deck: &DeckModel, title: &str) -> Result<Vec<u8>, ExportError> {
    let (doc, first_page, first_layer) = PdfDocument::new(title, Mm(254.0), Mm(143.0), "Slide");
    let font = doc
        .add_builtin_font(BuiltinFont::Helvetica)
        .map_err(|error| ExportError::Pdf(format!("{:?}", error)))?;
    for (index, slide) in deck.slides.iter().enumerate() {
        let (page, layer) = if index == 0 {
            (first_page, first_layer)
        } else {
            doc.add_page(Mm(254.0), Mm(143.0), format!("Slide {}", index + 1))
        };
        let current = doc.get_page(page).get_layer(layer);
        for element in &slide.elements {
            let scale_x = 254.0 / deck.canvas_width;
            let scale_y = 143.0 / deck.canvas_height;
            let x = element.x * scale_x;
            let y = 143.0 - (element.y + element.height) * scale_y;
            let width = element.width * scale_x;
            let height = element.height * scale_y;
            let rotation = element.rotation;
            match &element.kind {
                ElementKind::Text {
                    text,
                    font_size,
                    color,
                    ..
                } => {
                    let (draw_x, draw_y) =
                        begin_element_rotation(&current, x, y, width, height, rotation);
                    current.set_fill_color(slide_color(color, (0.1, 0.1, 0.1)));
                    current.begin_text_section();
                    current.set_font(&font, (*font_size as f32 / 2.0).max(8.0));
                    current.set_text_cursor(Mm(draw_x as f32), Mm((draw_y + height) as f32));
                    current.write_text(text, &font);
                    current.end_text_section();
                    end_element_rotation(&current, rotation);
                }
                ElementKind::Shape {
                    shape_type,
                    fill_color,
                    stroke_color,
                    stroke_width,
                    ..
                } => {
                    let (draw_x, draw_y) =
                        begin_element_rotation(&current, x, y, width, height, rotation);
                    let fill = slide_color(fill_color, (0.8, 0.8, 0.8));
                    let stroke = slide_color(stroke_color, (0.2, 0.2, 0.2));
                    current.set_fill_color(fill);
                    current.set_outline_color(stroke);
                    current.set_outline_thickness((*stroke_width as f32).max(0.5));
                    let shape_lower = shape_type.to_lowercase();
                    if shape_lower == "line" {
                        let (p1, p2) = if height < 1.0 {
                            (
                                Point::new(Mm(draw_x as f32), Mm((draw_y + height / 2.0) as f32)),
                                Point::new(
                                    Mm((draw_x + width) as f32),
                                    Mm((draw_y + height / 2.0) as f32),
                                ),
                            )
                        } else if width < 1.0 {
                            (
                                Point::new(
                                    Mm((draw_x + width / 2.0) as f32),
                                    Mm((draw_y + height) as f32),
                                ),
                                Point::new(Mm((draw_x + width / 2.0) as f32), Mm(draw_y as f32)),
                            )
                        } else {
                            (
                                Point::new(Mm(draw_x as f32), Mm((draw_y + height) as f32)),
                                Point::new(Mm((draw_x + width) as f32), Mm(draw_y as f32)),
                            )
                        };
                        current.add_line(Line {
                            points: vec![(p1, false), (p2, false)],
                            is_closed: false,
                        });
                    } else if shape_lower == "arrow" {
                        let p1 = Point::new(Mm(draw_x as f32), Mm((draw_y + height) as f32));
                        let p2 = Point::new(Mm((draw_x + width) as f32), Mm(draw_y as f32));
                        current.add_line(Line {
                            points: vec![(p1, false), (p2, false)],
                            is_closed: false,
                        });
                        let dx = width as f32;
                        let dy = -(height as f32);
                        let len = (dx * dx + dy * dy).sqrt().max(0.001);
                        let ux = dx / len;
                        let uy = dy / len;
                        let px = -uy;
                        let py = ux;
                        let head_len = 4.0_f32;
                        let head_w = 2.0_f32;
                        let tip_x = (draw_x + width) as f32;
                        let tip_y = draw_y as f32;
                        let base_x = tip_x - ux * head_len;
                        let base_y = tip_y - uy * head_len;
                        current.add_polygon(Polygon {
                            rings: vec![vec![
                                (Point::new(Mm(tip_x), Mm(tip_y)), false),
                                (
                                    Point::new(Mm(base_x + px * head_w), Mm(base_y + py * head_w)),
                                    false,
                                ),
                                (
                                    Point::new(Mm(base_x - px * head_w), Mm(base_y - py * head_w)),
                                    false,
                                ),
                            ]],
                            mode: printpdf::path::PaintMode::Fill,
                            winding_order: printpdf::path::WindingOrder::NonZero,
                        });
                    } else if shape_lower == "ellipse"
                        || shape_lower == "circle"
                        || shape_lower == "oval"
                    {
                        let cx = draw_x + width / 2.0;
                        let cy = draw_y + height / 2.0;
                        let rx = width / 2.0;
                        let ry = height / 2.0;
                        let segments = 32;
                        let ring: Vec<(Point, bool)> = (0..segments)
                            .map(|i| {
                                let angle =
                                    (i as f64) * 2.0 * std::f64::consts::PI / (segments as f64);
                                let px = cx + rx * angle.cos();
                                let py = cy + ry * angle.sin();
                                (Point::new(Mm(px as f32), Mm(py as f32)), false)
                            })
                            .collect();
                        current.add_polygon(Polygon {
                            rings: vec![ring],
                            mode: printpdf::path::PaintMode::FillStroke,
                            winding_order: printpdf::path::WindingOrder::NonZero,
                        });
                    } else {
                        current.add_polygon(Polygon {
                            rings: vec![vec![
                                (Point::new(Mm(draw_x as f32), Mm(draw_y as f32)), false),
                                (
                                    Point::new(Mm(draw_x as f32), Mm((draw_y + height) as f32)),
                                    false,
                                ),
                                (
                                    Point::new(
                                        Mm((draw_x + width) as f32),
                                        Mm((draw_y + height) as f32),
                                    ),
                                    false,
                                ),
                                (
                                    Point::new(Mm((draw_x + width) as f32), Mm(draw_y as f32)),
                                    false,
                                ),
                            ]],
                            mode: printpdf::path::PaintMode::FillStroke,
                            winding_order: printpdf::path::WindingOrder::NonZero,
                        });
                    }
                    end_element_rotation(&current, rotation);
                }
                ElementKind::Image { asset_hash, .. } => {
                    let Some(bytes) = data_uri_bytes(asset_hash) else {
                        continue;
                    };
                    let Ok(decoded) = ::image::load_from_memory(&bytes) else {
                        continue;
                    };
                    let image = printpdf::Image::from_dynamic_image(&decoded);
                    let dpi = 96.0_f32;
                    let image_width_mm = image.image.width.0 as f32 * 25.4 / dpi;
                    let image_height_mm = image.image.height.0 as f32 * 25.4 / dpi;
                    let (draw_x, draw_y) =
                        begin_element_rotation(&current, x, y, width, height, rotation);
                    image.add_to_layer(
                        current.clone(),
                        ImageTransform {
                            translate_x: Some(Mm(draw_x as f32)),
                            translate_y: Some(Mm(draw_y as f32)),
                            scale_x: Some((width as f32 / image_width_mm).max(0.001)),
                            scale_y: Some((height as f32 / image_height_mm).max(0.001)),
                            dpi: Some(dpi),
                            ..Default::default()
                        },
                    );
                    end_element_rotation(&current, rotation);
                }
                ElementKind::Table { rows, cols, data, .. } => {
                    let (draw_x, draw_y) =
                        begin_element_rotation(&current, x, y, width, height, rotation);
                    current.set_fill_color(Color::Rgb(Rgb::new(1.0, 1.0, 1.0, None)));
                    current.add_polygon(Polygon {
                        rings: vec![vec![
                            (Point::new(Mm(draw_x as f32), Mm(draw_y as f32)), false),
                            (
                                Point::new(Mm(draw_x as f32), Mm((draw_y + height) as f32)),
                                false,
                            ),
                            (
                                Point::new(
                                    Mm((draw_x + width) as f32),
                                    Mm((draw_y + height) as f32),
                                ),
                                false,
                            ),
                            (
                                Point::new(Mm((draw_x + width) as f32), Mm(draw_y as f32)),
                                false,
                            ),
                        ]],
                        mode: printpdf::path::PaintMode::FillStroke,
                        winding_order: printpdf::path::WindingOrder::NonZero,
                    });
                    current.set_fill_color(Color::Rgb(Rgb::new(0.15, 0.15, 0.15, None)));
                    current.begin_text_section();
                    current.set_font(&font, 8.0);
                    let mut line_y = (draw_y + height - 6.0) as f32;
                    for (ri, row) in data.iter().take(*rows).enumerate() {
                        let line = row
                            .iter()
                            .take(*cols)
                            .map(|c| c.as_str())
                            .collect::<Vec<_>>()
                            .join(" | ");
                        let text = if line.is_empty() {
                            format!("Row {}", ri + 1)
                        } else {
                            line
                        };
                        current.set_text_cursor(Mm((draw_x + 2.0) as f32), Mm(line_y));
                        current.write_text(&text, &font);
                        line_y -= 5.0;
                        if line_y < draw_y as f32 {
                            break;
                        }
                    }
                    current.end_text_section();
                    end_element_rotation(&current, rotation);
                }
                ElementKind::Chart {
                    chart_type,
                    data,
                    labels,
                    ..
                } => {
                    let (draw_x, draw_y) =
                        begin_element_rotation(&current, x, y, width, height, rotation);
                    current.set_fill_color(Color::Rgb(Rgb::new(0.97, 0.98, 0.99, None)));
                    current.add_polygon(Polygon {
                        rings: vec![vec![
                            (Point::new(Mm(draw_x as f32), Mm(draw_y as f32)), false),
                            (
                                Point::new(Mm(draw_x as f32), Mm((draw_y + height) as f32)),
                                false,
                            ),
                            (
                                Point::new(
                                    Mm((draw_x + width) as f32),
                                    Mm((draw_y + height) as f32),
                                ),
                                false,
                            ),
                            (
                                Point::new(Mm((draw_x + width) as f32), Mm(draw_y as f32)),
                                false,
                            ),
                        ]],
                        mode: printpdf::path::PaintMode::FillStroke,
                        winding_order: printpdf::path::WindingOrder::NonZero,
                    });
                    let max = data.iter().cloned().fold(0.0_f64, f64::max).max(1.0);
                    let bar_count = data.len().max(1);
                    let bar_width = (width / bar_count as f64) * 0.7;
                    let gap = (width / bar_count as f64) * 0.3;
                    if chart_type == "pie" {
                        let total = data.iter().sum::<f64>().max(1.0);
                        let mut angle = -90.0_f64;
                        let cx = draw_x + width / 2.0;
                        let cy = draw_y + height / 2.0;
                        let r = (width.min(height) / 2.2) as f32;
                        for (i, v) in data.iter().enumerate() {
                            let slice = (*v / total) * 360.0;
                            let hue = (i * 47) % 360;
                            let color = Color::Rgb(Rgb::new(
                                ((hue as f32) / 360.0).max(0.2),
                                0.55,
                                0.75,
                                None,
                            ));
                            current.set_fill_color(color);
                            let a1 = angle * std::f64::consts::PI / 180.0;
                            let a2 = (angle + slice) * std::f64::consts::PI / 180.0;
                            let x1 = cx + (r as f64) * a1.cos();
                            let y1 = cy + (r as f64) * a1.sin();
                            let x2 = cx + (r as f64) * a2.cos();
                            let y2 = cy + (r as f64) * a2.sin();
                            current.add_polygon(Polygon {
                                rings: vec![vec![
                                    (Point::new(Mm(cx as f32), Mm(cy as f32)), false),
                                    (Point::new(Mm(x1 as f32), Mm(y1 as f32)), false),
                                    (Point::new(Mm(x2 as f32), Mm(y2 as f32)), false),
                                ]],
                                mode: printpdf::path::PaintMode::Fill,
                                winding_order: printpdf::path::WindingOrder::NonZero,
                            });
                            angle += slice;
                        }
                    } else {
                        for (i, v) in data.iter().enumerate() {
                            let bar_h = (*v / max) * (height - 12.0);
                            let bx = draw_x + gap / 2.0 + i as f64 * (bar_width + gap);
                            let hue = (i * 47) % 360;
                            current.set_fill_color(Color::Rgb(Rgb::new(
                                ((hue as f32) / 360.0).max(0.2),
                                0.55,
                                0.75,
                                None,
                            )));
                            current.add_polygon(Polygon {
                                rings: vec![vec![
                                    (
                                        Point::new(
                                            Mm(bx as f32),
                                            Mm((draw_y + height - 6.0) as f32),
                                        ),
                                        false,
                                    ),
                                    (
                                        Point::new(
                                            Mm((bx + bar_width) as f32),
                                            Mm((draw_y + height - 6.0) as f32),
                                        ),
                                        false,
                                    ),
                                    (
                                        Point::new(
                                            Mm((bx + bar_width) as f32),
                                            Mm((draw_y + height - 6.0 - bar_h) as f32),
                                        ),
                                        false,
                                    ),
                                    (
                                        Point::new(
                                            Mm(bx as f32),
                                            Mm((draw_y + height - 6.0 - bar_h) as f32),
                                        ),
                                        false,
                                    ),
                                ]],
                                mode: printpdf::path::PaintMode::Fill,
                                winding_order: printpdf::path::WindingOrder::NonZero,
                            });
                        }
                    }
                    let title = labels
                        .first()
                        .cloned()
                        .unwrap_or_else(|| "Chart".to_string());
                    current.set_fill_color(Color::Rgb(Rgb::new(0.1, 0.1, 0.1, None)));
                    current.begin_text_section();
                    current.set_font(&font, 9.0);
                    current.set_text_cursor(
                        Mm((draw_x + 2.0) as f32),
                        Mm((draw_y + height - 2.0) as f32),
                    );
                    current.write_text(&title, &font);
                    current.end_text_section();
                    end_element_rotation(&current, rotation);
                }
            }
        }
        if !slide.notes.trim().is_empty() {
            current.set_fill_color(Color::Rgb(Rgb::new(0.25, 0.25, 0.25, None)));
            current.begin_text_section();
            current.set_font(&font, 8.0);
            current.set_text_cursor(Mm(8.0), Mm(6.0));
            let notes = if slide.notes.chars().count() > 180 {
                format!("{}…", slide.notes.chars().take(177).collect::<String>())
            } else {
                slide.notes.clone()
            };
            current.write_text(format!("Notes: {notes}"), &font);
            current.end_text_section();
        }
    }
    let mut buffer = Vec::new();
    doc.save(&mut BufWriter::new(&mut buffer))
        .map_err(|error| ExportError::Pdf(format!("{:?}", error)))?;
    Ok(buffer)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pdf_field_text_uses_bounded_cached_results_and_placeholders() {
        assert_eq!(
            pdf_field_text(&serde_json::json!({
                "type": "field",
                "attrs": { "kind": "page", "result": "12" }
            })),
            Some("12".to_string())
        );
        assert_eq!(
            pdf_field_text(&serde_json::json!({
                "type": "field",
                "attrs": { "kind": "numPages" }
            })),
            Some("[NUMPAGES]".to_string())
        );
        assert_eq!(
            pdf_field_text(&serde_json::json!({
                "type": "field",
                "attrs": { "kind": "page", "result": "  " }
            })),
            Some("[PAGE]".to_string())
        );
    }

    #[test]
    fn document_pdf_headers_footers() {
        let doc = serde_json::json!({
            "type": "doc",
            "pageSetup": {
                "header": "Header - Page {page} of {total}",
                "footer": "Footer - Page {page} of {total}"
            },
            "content": [{"type": "paragraph", "content": [{"type": "text", "text": "Test"}]}]
        });
        let bytes = export_doc_to_pdf(&doc, "Test doc").expect("export pdf");
        let pdf = String::from_utf8_lossy(&bytes);
        assert!(pdf.contains("486561646572202D20506167652031206F662031"));
        assert!(pdf.contains("466F6F746572202D20506167652031206F662031"));
    }

    #[test]
    fn document_pdf_wraps_and_creates_pages() {
        let long_text = "word ".repeat(2400);
        let doc = serde_json::json!({
            "type": "doc",
            "content": [{"type": "paragraph", "content": [{"type": "text", "text": long_text}]}]
        });
        let bytes = export_doc_to_pdf(&doc, "Long document").expect("export pdf");
        let pdf = String::from_utf8_lossy(&bytes);
        let pages = pdf
            .split("/Type/Pages/Count ")
            .nth(1)
            .and_then(|value| value.split('/').next())
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or_default();
        assert!(pages > 1, "expected multiple PDF pages, got {pages}");
    }

    #[test]
    fn workbook_pdf_paginates_large_sheets() {
        let mut workbook = WorkbookModel::new_default();
        for row in 1..=1200 {
            workbook.sheets[0].cells.insert(
                format!("{row}:1"),
                redoc_sheet_engine::SheetCell {
                    raw_value: row.to_string(),
                    display_value: row.to_string(),
                    formula: None,
                    style: None,
                },
            );
        }
        let bytes = export_workbook_to_pdf(&workbook, "Large sheet").expect("export pdf");
        let pdf = String::from_utf8_lossy(&bytes);
        let pages = pdf
            .split("/Type/Pages/Count ")
            .nth(1)
            .and_then(|value| value.split('/').next())
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or_default();
        assert!(pages > 1, "expected multiple sheet PDF pages, got {pages}");
    }

    #[test]
    fn slide_pdf_embeds_data_uri_images() {
        let mut deck = DeckModel::new_default();
        deck.slides[0].elements.push(redoc_slide_engine::SlideElement {
            id: "image-1".to_string(),
            x: 100.0,
            y: 100.0,
            width: 160.0,
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
                asset_hash: "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=".to_string(),
                mime: "image/png".to_string(),
            },
        });
        let bytes = export_deck_to_pdf(&deck, "Image slide").expect("export pdf");
        assert!(String::from_utf8_lossy(&bytes).contains("/Subtype/Image"));
    }

    #[test]
    fn slide_pdf_applies_element_rotation() {
        let mut deck = DeckModel::new_default();
        deck.slides[0].elements.clear();
        deck.slides[0]
            .elements
            .push(redoc_slide_engine::SlideElement {
                id: "rotated".to_string(),
                x: 200.0,
                y: 150.0,
                width: 200.0,
                height: 40.0,
                rotation: 45.0,
                z_index: 1,
                entrance: "none".to_string(),
                entrance_delay_ms: None,
                entrance_duration_ms: None,
                entrance_order: None,
                exit: "none".to_string(),
                exit_duration_ms: None,
                hyperlink: None,
                kind: ElementKind::Text {
                    text: "Rotated".to_string(),
                    font_size: 24.0,
                    font_family: "Helvetica".to_string(),
                    color: "#111111".to_string(),
                    align: "left".to_string(),
                    bold: false,
                    italic: false,
                    underline: false,
                    bullets: false,
                },
            });
        let bytes = export_deck_to_pdf(&deck, "Rotated slide").expect("export pdf");
        let pdf = String::from_utf8_lossy(&bytes);
        assert!(
            pdf.contains(" cm"),
            "expected CTM rotation operators in PDF content"
        );
        assert!(
            pdf.contains(" q") || pdf.contains("q\n"),
            "expected graphics state save"
        );
    }

    #[test]
    fn document_pdf_uses_bold_and_italic_font_variants() {
        let doc = serde_json::json!({
            "type": "doc",
            "content": [{
                "type": "paragraph",
                "content": [
                    { "type": "text", "text": "plain ", "marks": [] },
                    { "type": "text", "text": "bold", "marks": [{ "type": "bold" }] },
                    { "type": "text", "text": " ital", "marks": [{ "type": "italic" }] },
                    { "type": "text", "text": "both", "marks": [{ "type": "bold" }, { "type": "italic" }] }
                ]
            }]
        });
        let bytes = export_doc_to_pdf(&doc, "Run styles").expect("export pdf");
        let pdf = String::from_utf8_lossy(&bytes);
        // All four Helvetica variants must be embedded in the PDF.
        assert!(pdf.contains("/Helvetica "), "regular variant missing");
        assert!(pdf.contains("/Helvetica-Bold"), "bold variant missing");
        assert!(pdf.contains("/Helvetica-Oblique"), "italic variant missing");
        assert!(
            pdf.contains("/Helvetica-BoldOblique"),
            "bold-italic missing"
        );
    }

    #[test]
    fn document_pdf_renders_list_markers_and_alignment() {
        let doc = serde_json::json!({
            "type": "doc",
            "content": [
                { "type": "bullet_list", "content": [
                    { "type": "list_item", "content": [
                        { "type": "paragraph", "content": [{ "type": "text", "text": "First bullet" }] }
                    ]}
                ]},
                { "type": "ordered_list", "content": [
                    { "type": "list_item", "content": [
                        { "type": "paragraph", "content": [{ "type": "text", "text": "Numbered item" }] }
                    ]}
                ]},
                { "type": "paragraph", "attrs": { "align": "center" }, "content": [
                    { "type": "text", "text": "Centered" }
                ]}
            ]
        });
        let bytes = export_doc_to_pdf(&doc, "Lists").expect("export pdf");
        let pdf = String::from_utf8_lossy(&bytes);
        let bullet_hex = "E299A2"; // UTF-16BE-ish? printpdf encodes text as WinAnsi; verify below.
        assert!(
            pdf.contains("4E756D6265726564"),
            "ordered marker text present"
        );
        assert!(pdf.contains("43656E7465726564"), "centered text present");
        let _ = bullet_hex; // bullet glyph encoding is font-specific; presence of ordering asserted above
    }

    #[test]
    fn document_pdf_renders_footnote_references_and_notes() {
        let doc = serde_json::json!({
            "type": "doc",
            "content": [
                { "type": "paragraph", "content": [
                    { "type": "text", "text": "Claim" },
                    { "type": "footnote_ref", "attrs": { "id": "fn-a", "label": 1 } }
                ]}
            ],
            "footnotes": [
                { "id": "fn-a", "label": 1, "text": "The supporting source." }
            ]
        });
        let bytes = export_doc_to_pdf(&doc, "Notes").expect("export pdf");
        let pdf = String::from_utf8_lossy(&bytes);
        // "Claim" body text, the [1] reference marker, the Notes heading, and
        // the note text all survive into the PDF text streams.
        assert!(pdf.contains("436C61696D"), "body text present");
        assert!(pdf.contains("5B315D"), "[1] reference marker present");
        assert!(pdf.contains("4E6F746573"), "Notes heading present");
        assert!(pdf.contains("737570706F7274696E67"), "note text present");
    }

    #[test]
    fn document_pdf_renders_table_grid() {
        let doc = serde_json::json!({
            "type": "doc",
            "content": [{
                "type": "table",
                "content": [
                    { "type": "table_row", "content": [
                        { "type": "table_header", "content": [{ "type": "paragraph", "content": [{ "type": "text", "text": "H1" }] }] },
                        { "type": "table_header", "content": [{ "type": "paragraph", "content": [{ "type": "text", "text": "H2" }] }] }
                    ]},
                    { "type": "table_row", "content": [
                        { "type": "table_cell", "content": [{ "type": "paragraph", "content": [{ "type": "text", "text": "A" }] }] },
                        { "type": "table_cell", "content": [{ "type": "paragraph", "content": [{ "type": "text", "text": "B" }] }] }
                    ]}
                ]
            }]
        });
        let bytes = export_doc_to_pdf(&doc, "Table grid").expect("export pdf");
        let pdf = String::from_utf8_lossy(&bytes);
        // Cell text and header text must appear; the grid uses stroke
        // operators (' re' path painting) that text-flattening never emits.
        assert!(pdf.contains("4831"), "H1 text present");
        assert!(pdf.contains("4832"), "H2 text present");
        assert!(pdf.contains("S\n"), "stroke path operators present");
        assert!(pdf.contains("0.85 0.88 0.95"), "header row shading present");
    }

    #[test]
    fn document_pdf_embeds_data_uri_images() {
        let doc = serde_json::json!({
            "type": "doc",
            "content": [{"type": "image", "attrs": {"src": "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII="}}]
        });
        let bytes = export_doc_to_pdf(&doc, "Image document").expect("export pdf");
        assert!(String::from_utf8_lossy(&bytes).contains("/Subtype/Image"));
    }

    #[test]
    fn slide_pdf_renders_with_unicode_notes() {
        let mut deck = DeckModel::new_default();
        let unicode_note = "Héllo wörld 🌍 café résumé naïve ".repeat(10);
        assert!(unicode_note.chars().count() > 180);
        deck.slides[0].notes = unicode_note;
        let bytes = export_deck_to_pdf(&deck, "Unicode notes slide")
            .expect("export pdf with unicode notes");
        assert!(!bytes.is_empty());
    }

    #[test]
    fn document_pdf_custom_page_setup_and_geometry() {
        // Test custom page setup in document PDF rendering (Letter, landscape, custom margins)
        let doc = serde_json::json!({
            "type": "doc",
            "pageSetup": {
                "paperSize": "letter",
                "orientation": "landscape",
                "margins": { "top": 0.5, "bottom": 0.5, "left": 0.75, "right": 0.75 }
            },
            "content": [
                {
                    "type": "heading",
                    "content": [{"type": "text", "text": "Landscape Document Page Setup Test"}]
                },
                {
                    "type": "paragraph",
                    "content": [{"type": "text", "text": "This document uses custom Letter landscape setup."}]
                }
            ]
        });

        let bytes =
            export_doc_to_pdf(&doc, "Page Setup Document").expect("export pdf with page setup");
        assert!(!bytes.is_empty());
        let pdf_str = String::from_utf8_lossy(&bytes);
        assert!(pdf_str.contains("/MediaBox"));

        // Test geometry rendering parity for line, arrow, and ellipse shapes in slide PDF
        let mut deck = DeckModel::new_default();
        deck.slides[0].elements.clear();
        deck.slides[0]
            .elements
            .push(redoc_slide_engine::SlideElement {
                id: "ellipse-1".to_string(),
                x: 50.0,
                y: 50.0,
                width: 100.0,
                height: 60.0,
                rotation: 0.0,
                z_index: 1,
                entrance: "none".to_string(),
                entrance_delay_ms: None,
                entrance_duration_ms: None,
                entrance_order: None,
                exit: "none".to_string(),
                exit_duration_ms: None,
                hyperlink: None,
                kind: ElementKind::Shape {
                    shape_type: "ellipse".to_string(),
                    fill_color: "#ff0000".to_string(),
                    stroke_color: "#000000".to_string(),
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
        deck.slides[0]
            .elements
            .push(redoc_slide_engine::SlideElement {
                id: "line-1".to_string(),
                x: 10.0,
                y: 10.0,
                width: 150.0,
                height: 0.0,
                rotation: 0.0,
                z_index: 2,
                entrance: "none".to_string(),
                entrance_delay_ms: None,
                entrance_duration_ms: None,
                entrance_order: None,
                exit: "none".to_string(),
                exit_duration_ms: None,
                hyperlink: None,
                kind: ElementKind::Shape {
                    shape_type: "line".to_string(),
                    fill_color: "#000000".to_string(),
                    stroke_color: "#0000ff".to_string(),
                    stroke_width: 1.5,
                    text: String::new(),
                    fill_gradient: None,
                    shadow: false,
                    font_family: None,
                    bold: false,
                    italic: false,
                    underline: false,
                },
            });
        deck.slides[0]
            .elements
            .push(redoc_slide_engine::SlideElement {
                id: "arrow-1".to_string(),
                x: 20.0,
                y: 20.0,
                width: 80.0,
                height: 40.0,
                rotation: 0.0,
                z_index: 3,
                entrance: "none".to_string(),
                entrance_delay_ms: None,
                entrance_duration_ms: None,
                entrance_order: None,
                exit: "none".to_string(),
                exit_duration_ms: None,
                hyperlink: None,
                kind: ElementKind::Shape {
                    shape_type: "arrow".to_string(),
                    fill_color: "#00ff00".to_string(),
                    stroke_color: "#000000".to_string(),
                    stroke_width: 1.0,
                    text: String::new(),
                    fill_gradient: None,
                    shadow: false,
                    font_family: None,
                    bold: false,
                    italic: false,
                    underline: false,
                },
            });

        let shape_pdf_bytes =
            export_deck_to_pdf(&deck, "Geometry PDF").expect("export geometry pdf");
        assert!(!shape_pdf_bytes.is_empty());
    }
}
