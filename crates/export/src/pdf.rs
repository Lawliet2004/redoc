use printpdf::*;
use redoc_sheet_engine::WorkbookModel;
use redoc_slide_engine::{DeckModel, ElementKind};
use std::io::BufWriter;
use thiserror::Error;

fn decode_base64(value: &str) -> Option<Vec<u8>> {
    let mut output = Vec::new();
    let mut buffer = 0u32;
    let mut bits = 0u8;
    for byte in value.bytes().filter(|byte| !byte.is_ascii_whitespace()) {
        if byte == b'=' {
            break;
        }
        let value = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return None,
        } as u32;
        buffer = (buffer << 6) | value;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            output.push((buffer >> bits) as u8);
            buffer &= (1 << bits) - 1;
        }
    }
    Some(output)
}

fn data_uri_bytes(value: &str) -> Option<Vec<u8>> {
    let (_, encoded) = value.split_once(',')?;
    value.starts_with("data:").then(|| decode_base64(encoded))?
}

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
    let mut pages = vec![(page, layer)];
    let font = doc
        .add_builtin_font(BuiltinFont::Helvetica)
        .map_err(|e| ExportError::Pdf(format!("{:?}", e)))?;

    let printable_width = (page_width - margin_left - margin_right).max(10.0);
    let chars_per_line = ((printable_width * 92.0 / 170.0) as usize).max(20);
    let initial_y = page_height - margin_top - 10.0;

    fn text_content(node: &serde_json::Value) -> String {
        if node.get("type").and_then(|value| value.as_str()) == Some("text") {
            return node
                .get("text")
                .and_then(|value| value.as_str())
                .unwrap_or_default()
                .to_string();
        }
        node.get("content")
            .and_then(|value| value.as_array())
            .map(|children| {
                children
                    .iter()
                    .map(text_content)
                    .collect::<Vec<_>>()
                    .join("")
            })
            .unwrap_or_default()
    }

    enum Block {
        Text(String, bool, f32),
        Image(Vec<u8>),
        PageBreak,
    }

    fn collect_blocks(node: &serde_json::Value, blocks: &mut Vec<Block>) {
        let node_type = node.get("type").and_then(|value| value.as_str());
        if node_type == Some("page_break") {
            blocks.push(Block::PageBreak);
            return;
        }
        if node_type == Some("image") {
            if let Some(src) = node
                .get("attrs")
                .and_then(|attrs| attrs.get("src"))
                .and_then(|src| src.as_str())
            {
                if let Some(bytes) = data_uri_bytes(src) {
                    blocks.push(Block::Image(bytes));
                }
            }
            return;
        }
        if matches!(
            node_type,
            Some("paragraph" | "heading" | "blockquote" | "code_block")
        ) {
            let text = text_content(node).trim().to_string();
            if !text.is_empty() {
                let size = if node_type == Some("heading") {
                    16.0
                } else {
                    // Prefer explicit fontSize mark on first text child when present.
                    node.get("content")
                        .and_then(|c| c.as_array())
                        .and_then(|children| {
                            children.iter().find_map(|child| {
                                child
                                    .get("marks")
                                    .and_then(|m| m.as_array())
                                    .and_then(|marks| {
                                        marks.iter().find_map(|mark| {
                                            if mark.get("type").and_then(|t| t.as_str())
                                                == Some("fontSize")
                                            {
                                                mark.get("attrs")
                                                    .and_then(|a| a.get("size"))
                                                    .and_then(|s| {
                                                        s.as_f64()
                                                            .or_else(|| s.as_str()?.parse().ok())
                                                    })
                                                    .map(|v| v as f32)
                                            } else {
                                                None
                                            }
                                        })
                                    })
                            })
                        })
                        .unwrap_or(11.0)
                };
                blocks.push(Block::Text(text, node_type == Some("heading"), size));
            }
            return;
        }
        if node_type == Some("table") {
            if let Some(rows) = node.get("content").and_then(|value| value.as_array()) {
                for row in rows {
                    let cells = row
                        .get("content")
                        .and_then(|value| value.as_array())
                        .map(|cells| {
                            cells
                                .iter()
                                .map(|cell| text_content(cell).trim().to_string())
                                .collect::<Vec<_>>()
                                .join(" | ")
                        })
                        .unwrap_or_default();
                    if !cells.is_empty() {
                        blocks.push(Block::Text(cells, false, 11.0));
                    }
                }
            }
            return;
        }
        if let Some(children) = node.get("content").and_then(|value| value.as_array()) {
            for child in children {
                collect_blocks(child, blocks);
            }
        }
    }

    fn wrapped_lines(text: &str, width: usize) -> Vec<String> {
        let mut lines = Vec::new();
        let mut line = String::new();
        for word in text.split_whitespace() {
            if !line.is_empty() && line.len() + 1 + word.len() > width {
                lines.push(std::mem::take(&mut line));
            }
            if !line.is_empty() {
                line.push(' ');
            }
            line.push_str(word);
        }
        if !line.is_empty() {
            lines.push(line);
        }
        lines
    }

    let mut blocks = vec![Block::Text(format!("Document: {title}"), true, 16.0)];
    collect_blocks(doc_json, &mut blocks);
    let mut y = initial_y;
    let mut page_number = 1;
    for block in blocks {
        match block {
            Block::PageBreak => {
                page_number += 1;
                let next = doc.add_page(
                    Mm(page_width),
                    Mm(page_height),
                    format!("Page {page_number}"),
                );
                page = next.0;
                layer = next.1;
                pages.push((page, layer));
                y = initial_y;
            }
            Block::Text(text, heading, font_size) => {
                for line in wrapped_lines(&text, chars_per_line) {
                    if y < margin_bottom + 10.0 {
                        page_number += 1;
                        let next = doc.add_page(
                            Mm(page_width),
                            Mm(page_height),
                            format!("Page {page_number}"),
                        );
                        page = next.0;
                        layer = next.1;
                        pages.push((page, layer));
                        y = initial_y;
                    }
                    let current_layer = doc.get_page(page).get_layer(layer);
                    current_layer.begin_text_section();
                    current_layer.set_font(&font, font_size);
                    current_layer.set_text_cursor(Mm(margin_left), Mm(y));
                    current_layer.write_text(line, &font);
                    current_layer.end_text_section();
                    y -= if heading {
                        9.0
                    } else {
                        (font_size * 0.55).max(5.0)
                    };
                }
                y -= 4.0;
            }
            Block::Image(bytes) => {
                let Ok(decoded) = ::image::load_from_memory(&bytes) else {
                    continue;
                };
                let image = printpdf::Image::from_dynamic_image(&decoded);
                let dpi = 96.0_f32;
                let source_width = image.image.width.0 as f32 * 25.4 / dpi;
                let source_height = image.image.height.0 as f32 * 25.4 / dpi;
                let width = source_width.min(printable_width);
                let height = (source_height * (width / source_width.max(1.0))).min(100.0);
                if y - height < margin_bottom + 10.0 {
                    page_number += 1;
                    let next = doc.add_page(
                        Mm(page_width),
                        Mm(page_height),
                        format!("Page {page_number}"),
                    );
                    page = next.0;
                    layer = next.1;
                    y = initial_y;
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
            current_layer.set_font(&font, 10.0);
            current_layer.set_text_cursor(Mm(center_x), Mm(page_height - margin_top / 2.0));
            current_layer.write_text(h_text, &font);
            current_layer.end_text_section();
        }
        if let Some(ref footer) = footer_opt {
            let f_text = footer
                .replace("{page}", &(i + 1).to_string())
                .replace("{total}", &total_pages.to_string());
            let approx_width = f_text.chars().count() as f32 * 10.0 * 0.5;
            let center_x = (page_width - approx_width) / 2.0;
            current_layer.begin_text_section();
            current_layer.set_font(&font, 10.0);
            current_layer.set_text_cursor(Mm(center_x), Mm(margin_bottom / 2.0));
            current_layer.write_text(f_text, &font);
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
                _ => {}
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
                kind: ElementKind::Shape {
                    shape_type: "ellipse".to_string(),
                    fill_color: "#ff0000".to_string(),
                    stroke_color: "#000000".to_string(),
                    stroke_width: 2.0,
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
                kind: ElementKind::Shape {
                    shape_type: "line".to_string(),
                    fill_color: "#000000".to_string(),
                    stroke_color: "#0000ff".to_string(),
                    stroke_width: 1.5,
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
                kind: ElementKind::Shape {
                    shape_type: "arrow".to_string(),
                    fill_color: "#00ff00".to_string(),
                    stroke_color: "#000000".to_string(),
                    stroke_width: 1.0,
                },
            });

        let shape_pdf_bytes =
            export_deck_to_pdf(&deck, "Geometry PDF").expect("export geometry pdf");
        assert!(!shape_pdf_bytes.is_empty());
    }
}
