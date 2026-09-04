//! In-app export fixups (Phase 1 Fidelity).
//!
//! The compatibility inspector in `compatibility.rs` stays the source of
//! truth for *warnings*. This module implements the matching *fixups* that
//! the UI applies before export:
//!
//! - remote `http(s)` images are queued for client-side download to data-URIs
//!   (the actual fetch happens in TS glue; Rust only records the notice),
//! - custom paper sizes map to the nearest natively supported size and the
//!   mapping is persisted back into the document body,
//! - GIF/WebP inline images are transcoded to PNG data-URIs,
//! - radar (and other non-native) chart types convert to editable bar charts,
//! - unknown slide shapes convert to editable rectangles with the original
//!   shape name preserved as alt-text.

use serde_json::Value;
use std::io::Cursor;

#[derive(Debug, Clone)]
pub struct ExportFixupReport {
    pub body: Value,
    pub applied: Vec<String>,
    pub warnings: Vec<String>,
}

/// Supported paper sizes with twips dimensions (portrait).
const SUPPORTED_PAPERS: &[(&str, u32, u32)] = &[
    ("letter", 12240, 15840),
    ("a4", 11906, 16838),
    ("legal", 12240, 20160),
    ("executive", 10440, 15120),
];

/// Well-known custom sizes resolved to the nearest supported paper by area.
const CUSTOM_PAPER_DIMS: &[(&str, u32, u32)] = &[
    ("a3", 16838, 23811),
    ("a5", 8419, 11906),
    ("tabloid", 15840, 24480),
    ("ledger", 24480, 15840),
    ("b4", 14337, 20365),
    ("b5", 10359, 14655),
];

/// Map any paper name to the nearest natively supported size.
/// Returns `(nearest, was_custom)`.
pub fn nearest_paper_size(requested: &str) -> (&'static str, bool) {
    let lower = requested.trim().to_ascii_lowercase();
    for (name, _, _) in SUPPORTED_PAPERS {
        if lower == *name {
            return (name, false);
        }
    }
    let area = |w: u32, h: u32| w as u64 * h as u64;
    let custom_area = CUSTOM_PAPER_DIMS
        .iter()
        .find(|(name, _, _)| lower.contains(name))
        .map(|(_, w, h)| area(*w, *h));
    let mut best = "letter";
    let mut best_diff = u64::MAX;
    for (name, w, h) in SUPPORTED_PAPERS {
        let candidate = area(*w, *h);
        let diff = custom_area
            .map(|known| candidate.abs_diff(known))
            .unwrap_or(u64::MAX);
        // Without dimensions fall back deterministically to letter.
        if diff < best_diff {
            best_diff = diff;
            best = name;
        }
    }
    (best, true)
}

fn is_remote_src(src: &str) -> bool {
    let lower = src.trim().to_ascii_lowercase();
    lower.starts_with("http://") || lower.starts_with("https://")
}

fn data_uri_mime(value: &str) -> Option<String> {
    let (header, _) = value.split_once(',')?;
    if !value.starts_with("data:") {
        return None;
    }
    Some(
        header
            .strip_prefix("data:")?
            .split(';')
            .next()?
            .trim()
            .to_ascii_lowercase(),
    )
}

/// Transcode a GIF/WebP data-URI to a PNG data-URI. Returns `None` when the
/// input is not a GIF/WebP data-URI or cannot be decoded.
pub fn transcode_image_to_png_data_uri(value: &str) -> Option<String> {
    let mime = data_uri_mime(value)?;
    if mime != "image/gif" && mime != "image/webp" {
        return None;
    }
    let bytes = crate::base64_util::data_uri_bytes(value)?;
    if bytes.len() > 8 * 1024 * 1024 {
        return None;
    }
    let image = ::image::load_from_memory(&bytes).ok()?;
    let mut png = Vec::new();
    image
        .write_to(&mut Cursor::new(&mut png), ::image::ImageFormat::Png)
        .ok()?;
    Some(format!(
        "data:image/png;base64,{}",
        crate::base64_util::encode_base64(&png)
    ))
}

fn page_setup_mut(body: &mut Value) -> Option<&mut serde_json::Map<String, Value>> {
    if body.get("pageSetup").is_some() {
        return body.get_mut("pageSetup")?.as_object_mut();
    }
    if body.get("page_setup").is_some() {
        return body.get_mut("page_setup")?.as_object_mut();
    }
    None
}

fn fixup_doc_paper(body: &mut Value, applied: &mut Vec<String>) {
    let setup = match page_setup_mut(body) {
        Some(setup) => setup,
        None => return,
    };
    let requested = setup
        .get("paperSize")
        .or_else(|| setup.get("paper_size"))
        .and_then(Value::as_str)
        .unwrap_or("letter")
        .to_string();
    let (nearest, was_custom) = nearest_paper_size(&requested);
    if was_custom {
        setup.insert("paperSize".to_string(), Value::String(nearest.to_string()));
        setup.remove("paper_size");
        applied.push(format!(
            "Custom paper '{requested}' mapped to nearest supported size '{nearest}' and saved to the document."
        ));
    }
}

fn visit_doc_images(body: &mut Value, applied: &mut Vec<String>, remote: &mut Vec<String>) {
    match body {
        Value::Object(map) => {
            if map.get("type").and_then(Value::as_str) == Some("image") {
                if let Some(attrs) = map.get_mut("attrs").and_then(Value::as_object_mut) {
                    if let Some(src) = attrs.get("src").and_then(Value::as_str).map(str::to_string)
                    {
                        if is_remote_src(&src) {
                            remote.push(src);
                        } else if let Some(png) = transcode_image_to_png_data_uri(&src) {
                            attrs.insert("src".to_string(), Value::String(png));
                            applied.push(
                                "GIF/WebP document image transcoded to PNG for native export."
                                    .to_string(),
                            );
                        }
                    }
                }
            }
            for value in map.values_mut() {
                visit_doc_images(value, applied, remote);
            }
        }
        Value::Array(items) => {
            for item in items {
                visit_doc_images(item, applied, remote);
            }
        }
        _ => {}
    }
}

fn fixup_workbook(body: &mut Value, applied: &mut Vec<String>, remote: &mut Vec<String>) {
    let mut workbook: redoc_sheet_engine::WorkbookModel =
        match serde_json::from_value(body.clone()) {
            Ok(workbook) => workbook,
            Err(_) => return,
        };
    for sheet in &mut workbook.sheets {
        for chart in &mut sheet.charts {
            let lower = chart.chart_type.to_ascii_lowercase();
            if !matches!(lower.as_str(), "bar" | "line" | "pie") {
                let original = chart.chart_type.clone();
                chart.chart_type = "bar".to_string();
                if original.eq_ignore_ascii_case("radar") {
                    applied.push(format!(
                        "Radar chart on '{}' converted to an editable bar chart.",
                        sheet.name
                    ));
                } else {
                    applied.push(format!(
                        "Chart type '{original}' on '{}' converted to an editable bar chart.",
                        sheet.name
                    ));
                }
            }
        }
        for cell in sheet.cells.values_mut() {
            if let Some(style) = cell.style.as_mut() {
                if let Some(image) = style.image.clone() {
                    if is_remote_src(&image) {
                        remote.push(image);
                    } else if let Some(png) = transcode_image_to_png_data_uri(&image) {
                        style.image = Some(png);
                        applied.push(
                            "GIF/WebP cell image transcoded to PNG for native XLSX drawings."
                                .to_string(),
                        );
                    }
                }
            }
        }
    }
    if let Ok(fixed) = serde_json::to_value(&workbook) {
        *body = fixed;
    }
}

fn fixup_deck(body: &mut Value, applied: &mut Vec<String>, remote: &mut Vec<String>) {
    use redoc_slide_engine::ElementKind;
    let mut deck: redoc_slide_engine::DeckModel = match serde_json::from_value(body.clone()) {
        Ok(deck) => deck,
        Err(_) => return,
    };
    for slide in &mut deck.slides {
        for element in &mut slide.elements {
            match &mut element.kind {
                ElementKind::Chart { chart_type, .. } => {
                    let lower = chart_type.to_ascii_lowercase();
                    if !matches!(lower.as_str(), "bar" | "column" | "line" | "pie") {
                        let original = chart_type.clone();
                        *chart_type = "bar".to_string();
                        applied.push(format!(
                            "Radar chart '{original}' converted to a native editable bar chart."
                        ));
                    }
                }
                ElementKind::Shape {
                    shape_type,
                    text,
                    ..
                } => {
                    let lower = shape_type.to_ascii_lowercase();
                    let known = matches!(
                        lower.as_str(),
                        "rect"
                            | "roundedrect"
                            | "roundrect"
                            | "ellipse"
                            | "triangle"
                            | "diamond"
                            | "star"
                            | "star5"
                            | "line"
                            | "arrow"
                    );
                    if !known {
                        let original = shape_type.clone();
                        *shape_type = "rect".to_string();
                        let alt = format!("[Original shape: {original}]");
                        if text.trim().is_empty() {
                            *text = alt.clone();
                        } else if !text.contains(&alt) {
                            text.push_str(&format!(" {alt}"));
                        }
                        applied.push(format!(
                            "Unknown shape '{original}' converted to an editable rectangle (original preserved as alt-text)."
                        ));
                    }
                }
                ElementKind::Image { asset_hash, .. } => {
                    if is_remote_src(asset_hash) {
                        remote.push(asset_hash.clone());
                    } else if let Some(png) = transcode_image_to_png_data_uri(asset_hash) {
                        *asset_hash = png;
                        applied.push(
                            "GIF/WebP slide image transcoded to PNG for native export.".to_string(),
                        );
                    }
                }
                _ => {}
            }
        }
    }
    if let Ok(fixed) = serde_json::to_value(&deck) {
        *body = fixed;
    }
}

/// Apply in-app fixups for an export path. Warning detection is unchanged
/// (see `compatibility.rs`); the returned `warnings` are recomputed against
/// the fixed body so the UI can show what, if anything, still needs review.
pub fn apply_export_fixups(mode: &str, format: &str, body: &Value) -> ExportFixupReport {
    let mut fixed = body.clone();
    let mut applied = Vec::new();
    let mut remote: Vec<String> = Vec::new();
    match (mode, format) {
        ("doc", "docx") => {
            fixup_doc_paper(&mut fixed, &mut applied);
            visit_doc_images(&mut fixed, &mut applied, &mut remote);
        }
        ("sheet", "xlsx") => fixup_workbook(&mut fixed, &mut applied, &mut remote),
        ("slide", "pptx") => fixup_deck(&mut fixed, &mut applied, &mut remote),
        _ => {}
    }
    remote.sort();
    remote.dedup();
    for src in remote.iter().take(5) {
        applied.push(format!(
            "Remote image '{src}' queued for in-app download to a data-URI before export."
        ));
    }
    if remote.len() > 5 {
        applied.push(format!(
            "…plus {} more remote image(s) queued for download.",
            remote.len() - 5
        ));
    }
    let warnings = crate::compatibility::export_compatibility_warnings(mode, format, &fixed);
    ExportFixupReport {
        body: fixed,
        applied,
        warnings,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_custom_paper_to_nearest_and_persists() {
        let body = serde_json::json!({
            "type": "doc",
            "pageSetup": { "paperSize": "tabloid", "orientation": "portrait" },
            "content": []
        });
        let report = apply_export_fixups("doc", "docx", &body);
        assert_eq!(report.body["pageSetup"]["paperSize"], "legal");
        assert!(report.applied.iter().any(|note| note.contains("tabloid")));
        // Warning for custom paper is gone after the fixup persisted.
        assert!(!report.warnings.iter().any(|w| w.contains("custom paper")));
    }

    #[test]
    fn keeps_supported_paper_untouched() {
        let body = serde_json::json!({
            "type": "doc",
            "pageSetup": { "paperSize": "a4" },
            "content": []
        });
        let report = apply_export_fixups("doc", "docx", &body);
        assert_eq!(report.body["pageSetup"]["paperSize"], "a4");
        assert!(report.applied.is_empty());
    }

    #[test]
    fn converts_radar_chart_to_bar_with_notice() {
        let workbook = serde_json::json!({
            "sheets": [{
                "id": "s1", "name": "Sheet1", "cells": {}, "colWidths": {},
                "rowHeights": {}, "freezeRows": 0, "freezeCols": 0,
                "charts": [{ "chartType": "radar", "startRow": 1, "endRow": 3, "startCol": 1, "endCol": 2 }]
            }],
            "activeSheetIndex": 0
        });
        let report = apply_export_fixups("sheet", "xlsx", &workbook);
        assert_eq!(report.body["sheets"][0]["charts"][0]["chartType"], "bar");
        assert!(report.applied.iter().any(|note| note.contains("Radar")));
    }

    #[test]
    fn converts_unknown_shape_to_rect_with_alt_text() {
        let deck = serde_json::json!({
            "slides": [{
                "id": "sl1", "layout": "blank", "notes": "", "bgOverride": null,
                "transition": "none",
                "elements": [{
                    "id": "e1", "x": 0.0, "y": 0.0, "width": 10.0, "height": 10.0,
                    "rotation": 0.0, "zIndex": 1, "entrance": "none", "exit": "none",
                    "kind": { "Shape": { "shapeType": "hexagon", "fillColor": "#fff",
                        "strokeColor": "#000", "strokeWidth": 1.0, "text": "Hi" } }
                }]
            }],
            "theme": { "id": "t", "name": "T", "bgColor": "#fff", "textColor": "#000",
                "accentColor": "#00f", "fontFamily": "Arial" },
            "canvasWidth": 960.0, "canvasHeight": 540.0, "activeSlideIndex": 0
        });
        let report = apply_export_fixups("slide", "pptx", &deck);
        let kind = &report.body["slides"][0]["elements"][0]["kind"];
        assert_eq!(kind["Shape"]["shapeType"], "rect");
        assert!(kind["Shape"]["text"]
            .as_str()
            .unwrap()
            .contains("hexagon"));
        assert!(report.applied.iter().any(|note| note.contains("hexagon")));
    }

    #[test]
    fn queues_remote_images_for_download() {
        let body = serde_json::json!({
            "type": "doc",
            "content": [{ "type": "image", "attrs": { "src": "https://example.invalid/a.png" } }]
        });
        let report = apply_export_fixups("doc", "docx", &body);
        assert!(report.applied.iter().any(|note| note.contains("queued")));
    }

    #[test]
    fn transcodes_gif_data_uri_to_png() {
        // 1x1 transparent GIF.
        let gif = "data:image/gif;base64,R0lGODlhAQABAIAAAP///////yH5BAEKAAEALAAAAAABAAEAAAICTAEAOw==";
        let transcoded = transcode_image_to_png_data_uri(gif).expect("transcode gif");
        assert!(transcoded.starts_with("data:image/png;base64,"));
        assert!(data_uri_mime(&transcoded).unwrap() == "image/png");
    }
}
