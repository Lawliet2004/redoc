use redoc_sheet_engine::WorkbookModel;
use redoc_slide_engine::{DeckModel, ElementKind};
use serde_json::Value;
use std::collections::BTreeSet;

fn push_once(warnings: &mut BTreeSet<String>, message: impl Into<String>) {
    warnings.insert(message.into());
}

fn visit_doc(node: &Value, warnings: &mut BTreeSet<String>) {
    let Some(object) = node.as_object() else {
        return;
    };
    match object.get("type").and_then(Value::as_str) {
        Some("doc")
        | Some("paragraph")
        | Some("heading")
        | Some("blockquote")
        | Some("bullet_list")
        | Some("ordered_list")
        | Some("list_item")
        | Some("table")
        | Some("table_row")
        | Some("table_cell")
        | Some("code_block")
        | Some("page_break")
        | Some("section_break")
        | Some("text")
        | Some("image") => {}
        Some("hard_break") => push_once(
            warnings,
            "DOCX: hard breaks are simplified to Word text-wrapping breaks.",
        ),
        Some("horizontal_rule") => push_once(
            warnings,
            "DOCX: horizontal rules are not emitted by the current writer exporter.",
        ),
        Some("table_of_contents") | Some("content_control") => push_once(
            warnings,
            "DOCX: advanced fields/content controls are not preserved.",
        ),
        Some("field") => {
            let kind = object
                .get("attrs")
                .and_then(|attrs| attrs.get("kind"))
                .and_then(Value::as_str)
                .unwrap_or_default();
            if !matches!(kind.to_ascii_lowercase().as_str(), "page" | "numpages") {
                push_once(
                    warnings,
                    "DOCX: unsupported document field codes may be omitted during export; use a supported PAGE or NUMPAGES field.",
                );
            }
        }
        Some(other) => push_once(
            warnings,
            format!("DOCX: unsupported node type '{other}' may be omitted."),
        ),
        None => {}
    }

    if object.get("type").and_then(Value::as_str) == Some("image") {
        let src = object
            .get("attrs")
            .and_then(|attrs| attrs.get("src"))
            .and_then(Value::as_str)
            .unwrap_or_default();
        if !src.starts_with("data:") {
            push_once(
                warnings,
                "DOCX: images without an inline data URI may be omitted.",
            );
        }
    }

    if let Some(marks) = object.get("marks").and_then(Value::as_array) {
        for mark in marks {
            match mark.get("type").and_then(Value::as_str) {
                Some(
                    "bold" | "italic" | "underline" | "strike" | "superscript" | "subscript"
                    | "fontFamily" | "fontSize" | "color" | "highlight" | "link" | "trackInsert"
                    | "trackDelete" | "bookmark",
                )
                | None => {}
                Some(other) => push_once(
                    warnings,
                    format!("DOCX: mark '{other}' is not preserved by the exporter."),
                ),
            }
        }
    }

    if let Some(children) = object.get("content").and_then(Value::as_array) {
        for child in children {
            visit_doc(child, warnings);
        }
    }
}

fn docx_warnings(body: &Value) -> Vec<String> {
    let mut warnings = BTreeSet::new();
    visit_doc(body, &mut warnings);
    if let Some(setup) = body
        .get("pageSetup")
        .or_else(|| body.get("page_setup"))
        .and_then(Value::as_object)
    {
        let paper = setup
            .get("paperSize")
            .or_else(|| setup.get("paper_size"))
            .and_then(Value::as_str)
            .unwrap_or("letter");
        if !matches!(
            paper.to_ascii_lowercase().as_str(),
            "letter" | "a4" | "legal" | "executive"
        ) {
            push_once(
                &mut warnings,
                "DOCX: custom paper sizes fall back to Letter during export.",
            );
        }
    }
    warnings.into_iter().collect()
}

fn xlsx_native_image_supported(value: &str) -> bool {
    let Some((header, _)) = value.split_once(',') else {
        return false;
    };
    let mime = header
        .strip_prefix("data:")
        .and_then(|value| value.split(';').next())
        .map(str::trim)
        .map(str::to_ascii_lowercase);
    if !matches!(
        mime.as_deref(),
        Some("image/png") | Some("image/jpeg") | Some("image/jpg")
    ) {
        return false;
    }
    let Some(bytes) = crate::base64_util::data_uri_bytes(value) else {
        return false;
    };
    bytes.len() <= 8 * 1024 * 1024 && ::image::load_from_memory(&bytes).is_ok()
}

fn xlsx_warnings(workbook: &WorkbookModel) -> Vec<String> {
    let mut warnings = BTreeSet::new();
    for sheet in &workbook.sheets {
        if sheet.cells.values().any(|cell| {
            cell.style
                .as_ref()
                .and_then(|style| style.image.as_deref())
                .is_some_and(|image| !xlsx_native_image_supported(image))
        }) {
            push_once(
                &mut warnings,
                "XLSX: inline cell images use native drawings only for valid PNG/JPEG data; other formats remain Redoc-only.",
            );
        }
        if !sheet.pivot_tables.is_empty() {
            push_once(
                &mut warnings,
                "XLSX: pivot tables export as native pivot parts; Excel refreshes the summary grid on open.",
            );
        }
        if !sheet.slicers.is_empty() && sheet.pivot_tables.is_empty() {
            push_once(
                &mut warnings,
                "XLSX: slicers are preserved in .redoc but native XLSX slicer parts are not emitted.",
            );
        }
        for chart in &sheet.charts {
            if !matches!(
                chart.chart_type.to_ascii_lowercase().as_str(),
                "bar" | "line" | "pie"
            ) {
                push_once(
                    &mut warnings,
                    format!(
                        "XLSX: chart type '{}' falls back to a bar chart.",
                        chart.chart_type
                    ),
                );
            }
        }
        if sheet.conditional_formatting.iter().any(|rule| {
            !matches!(
                rule.rule_type.as_str(),
                "greaterThan" | "lessThan" | "equalTo" | "textContains" | "dataBar" | "colorScale"
            )
        }) {
            push_once(
                &mut warnings,
                "XLSX: unsupported conditional-format rules are skipped.",
            );
        }
    }
    warnings.into_iter().collect()
}

fn pptx_warnings(deck: &DeckModel) -> Vec<String> {
    let mut warnings = BTreeSet::new();
    for slide in &deck.slides {
        if !matches!(
            slide.transition.as_str(),
            "none"
                | "fade"
                | "slide-left"
                | "slide-right"
                | "wipe-left"
                | "wipe-right"
                | "zoom"
                | "dissolve"
        ) {
            push_once(
                &mut warnings,
                format!(
                    "PPTX: slide transition '{}' falls back to no transition.",
                    slide.transition
                ),
            );
        }
        for element in &slide.elements {
            if let Some(target) = element.hyperlink.as_deref() {
                let lower = target.trim().to_ascii_lowercase();
                let safe_scheme = lower.starts_with("http://")
                    || lower.starts_with("https://")
                    || lower.starts_with("mailto:")
                    || lower.starts_with("tel:");
                if !safe_scheme || target.trim().is_empty() || target.chars().count() > 2_048 {
                    push_once(
                        &mut warnings,
                        "PPTX: unsafe or malformed hyperlink targets are omitted during export.",
                    );
                }
            }
            if !matches!(element.entrance.as_str(), "none" | "fade" | "zoom") {
                push_once(
                    &mut warnings,
                    format!(
                        "PPTX: entrance animation '{}' is not emitted.",
                        element.entrance
                    ),
                );
            }
            if !matches!(element.exit.as_str(), "none" | "fade") {
                push_once(
                    &mut warnings,
                    format!("PPTX: exit animation '{}' is not emitted.", element.exit),
                );
            }
            match &element.kind {
                ElementKind::Chart { chart_type, .. }
                    if !matches!(
                        chart_type.to_ascii_lowercase().as_str(),
                        "bar" | "column" | "line" | "pie"
                    ) =>
                {
                    push_once(
                        &mut warnings,
                        format!(
                            "PPTX: chart type '{}' exports as an editable summary shape, not a native chart part.",
                            chart_type
                        ),
                    );
                }
                ElementKind::Shape { shape_type, .. }
                    if !matches!(
                        shape_type.to_ascii_lowercase().as_str(),
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
                    ) =>
                {
                    push_once(
                        &mut warnings,
                        format!(
                            "PPTX: shape type '{}' falls back to a rectangle.",
                            shape_type
                        ),
                    );
                }
                ElementKind::Image { asset_hash, .. } if !asset_hash.starts_with("data:") => {
                    push_once(
                        &mut warnings,
                        "PPTX: images without an inline data URI export as warning placeholders.",
                    );
                }
                _ => {}
            }
        }
    }
    warnings.into_iter().collect()
}

pub fn export_compatibility_warnings(mode: &str, format: &str, body: &Value) -> Vec<String> {
    match (mode, format) {
        ("doc", "docx") => docx_warnings(body),
        ("sheet", "xlsx") => serde_json::from_value::<WorkbookModel>(body.clone())
            .map(|workbook| xlsx_warnings(&workbook))
            .unwrap_or_else(|_| {
                vec!["XLSX: workbook metadata could not be validated before export.".to_string()]
            }),
        ("slide", "pptx") => serde_json::from_value::<DeckModel>(body.clone())
            .map(|deck| pptx_warnings(&deck))
            .unwrap_or_else(|_| {
                vec!["PPTX: deck metadata could not be validated before export.".to_string()]
            }),
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use redoc_sheet_engine::{
        CellStyle, ChartModel, ScenarioCellChange, ScenarioModel, SheetCell, SheetData,
    };
    use redoc_slide_engine::{ElementKind, SlideElement};

    #[test]
    fn reports_docx_unsupported_nodes_and_images() {
        let body = serde_json::json!({
            "type": "doc",
            "content": [
                { "type": "horizontal_rule" },
                { "type": "image", "attrs": { "src": "https://example.invalid/image.png" } },
                { "type": "field", "attrs": { "kind": "TOC" } }
            ]
        });
        let warnings = export_compatibility_warnings("doc", "docx", &body);
        assert!(warnings
            .iter()
            .any(|warning| warning.contains("horizontal rules")));
        assert!(warnings
            .iter()
            .any(|warning| warning.contains("images without")));
        assert!(warnings
            .iter()
            .any(|warning| warning.contains("unsupported document field")));
    }

    #[test]
    fn accepts_supported_section_breaks_and_bookmark_marks() {
        let body = serde_json::json!({
            "type": "doc",
            "content": [
                { "type": "paragraph", "content": [{
                    "type": "text",
                    "text": "Target",
                    "marks": [{ "type": "bookmark", "attrs": { "name": "Target" } }]
                }] },
                { "type": "paragraph", "content": [
                    { "type": "field", "attrs": { "kind": "page", "result": "1" } },
                    { "type": "field", "attrs": { "kind": "numPages", "result": "2" } }
                ] },
                { "type": "section_break", "attrs": { "pageSetup": { "breakType": "nextPage" } } }
            ]
        });
        assert!(export_compatibility_warnings("doc", "docx", &body).is_empty());
    }

    #[test]
    fn reports_xlsx_pivot_and_unknown_chart() {
        let mut sheet = SheetData::new("sheet-1", "Sheet1");
        sheet.scenarios.push(ScenarioModel {
            id: "scenario-1".to_string(),
            name: "Test scenario".to_string(),
            changes: vec![ScenarioCellChange {
                row: 1,
                col: 1,
                raw_value: "42".to_string(),
            }],
        });
        sheet.charts.push(ChartModel {
            chart_type: "radar".to_string(),
            title: None,
            start_row: 1,
            end_row: 3,
            start_col: 1,
            end_col: 2,
        });
        sheet
            .pivot_tables
            .push(redoc_sheet_engine::PivotTableModel {
                id: "pivot-1".to_string(),
                source_range: redoc_sheet_engine::CellRange {
                    start_row: 1,
                    end_row: 3,
                    start_col: 1,
                    end_col: 2,
                },
                row_field: 1,
                column_field: None,
                value_field: 2,
                aggregation: "sum".to_string(),
                output_start_row: 5,
                output_start_col: 1,
                output_row_count: None,
                output_col_count: None,
            });
        let workbook = WorkbookModel {
            sheets: vec![sheet],
            active_sheet_index: 0,
            ..WorkbookModel::new_default()
        };
        let warnings = export_compatibility_warnings(
            "sheet",
            "xlsx",
            &serde_json::to_value(&workbook).expect("serialize workbook"),
        );
        assert!(warnings
            .iter()
            .any(|warning| warning.contains("native pivot parts")));
        assert!(warnings.iter().any(|warning| warning.contains("radar")));
        assert!(!warnings
            .iter()
            .any(|warning| warning.contains("scenario parts")));
    }

    #[test]
    fn reports_xlsx_slicer_fallback() {
        let mut sheet = SheetData::new("sheet-1", "Sheet1");
        sheet.slicers.push(redoc_sheet_engine::SlicerModel {
            id: "slicer-1".to_string(),
            title: "Region".to_string(),
            source_range: redoc_sheet_engine::CellRange {
                start_row: 1,
                end_row: 3,
                start_col: 1,
                end_col: 2,
            },
            column: 1,
            selected_values: vec!["East".to_string()],
        });
        let workbook = WorkbookModel {
            sheets: vec![sheet],
            active_sheet_index: 0,
            ..WorkbookModel::new_default()
        };
        let warnings = export_compatibility_warnings(
            "sheet",
            "xlsx",
            &serde_json::to_value(&workbook).expect("serialize workbook"),
        );
        assert!(warnings.iter().any(|warning| warning.contains("slicers")));
    }

    #[test]
    fn reports_xlsx_cell_image_fallback() {
        let mut sheet = SheetData::new("sheet-1", "Sheet1");
        sheet.cells.insert(
            "1:1".to_string(),
            SheetCell {
                raw_value: String::new(),
                display_value: String::new(),
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
                    image: Some("data:image/png;base64,aA==".to_string()),
                    ..Default::default()
                }),
            },
        );
        let workbook = WorkbookModel {
            sheets: vec![sheet],
            active_sheet_index: 0,
            ..WorkbookModel::new_default()
        };
        let warnings = export_compatibility_warnings(
            "sheet",
            "xlsx",
            &serde_json::to_value(&workbook).expect("serialize workbook"),
        );
        assert!(warnings
            .iter()
            .any(|warning| warning.contains("cell images")));

        let mut valid_workbook = workbook.clone();
        valid_workbook.sheets[0].cells.get_mut("1:1").unwrap().style.as_mut().unwrap().image =
            Some("data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=".to_string());
        let valid_warnings = export_compatibility_warnings(
            "sheet",
            "xlsx",
            &serde_json::to_value(valid_workbook).expect("serialize valid workbook"),
        );
        assert!(!valid_warnings
            .iter()
            .any(|warning| warning.contains("cell images")));
    }

    #[test]
    fn reports_pptx_fallbacks_and_unknown_animation() {
        let mut deck = DeckModel::new_default();
        deck.slides[0].transition = "spin".to_string();
        deck.slides[0].elements.push(SlideElement {
            id: "shape-1".to_string(),
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
            rotation: 0.0,
            z_index: 1,
            entrance: "spin".to_string(),
            entrance_delay_ms: None,
            entrance_duration_ms: None,
            entrance_order: None,
            exit: "none".to_string(),
            exit_duration_ms: None,
            hyperlink: None,
            kind: ElementKind::Chart {
                chart_type: "radar".to_string(),
                data: vec![1.0],
                labels: vec!["Value".to_string()],
                legend: true,
                show_labels: true,
                show_axes: true,
            },
        });
        deck.slides[0].elements.push(SlideElement {
            id: "triangle".to_string(),
            x: 20.0,
            y: 20.0,
            width: 40.0,
            height: 40.0,
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
                shape_type: "triangle".to_string(),
                fill_color: "#ffffff".to_string(),
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
        deck.slides[0].elements.push(SlideElement {
            id: "hexagon".to_string(),
            x: 70.0,
            y: 20.0,
            width: 40.0,
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
                shape_type: "hexagon".to_string(),
                fill_color: "#ffffff".to_string(),
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
        let warnings = export_compatibility_warnings(
            "slide",
            "pptx",
            &serde_json::to_value(deck).expect("serialize deck"),
        );
        assert!(warnings
            .iter()
            .any(|warning| warning.contains("transition")));
        assert!(warnings
            .iter()
            .any(|warning| warning.contains("entrance animation")));
        assert!(warnings
            .iter()
            .any(|warning| warning.contains("chart type")));
        assert!(warnings
            .iter()
            .any(|warning| warning.contains("shape type 'hexagon'")));
        assert!(!warnings
            .iter()
            .any(|warning| warning.contains("shape type 'triangle'")));
    }

    #[test]
    fn accepts_supported_pptx_transitions_without_warnings() {
        for transition in [
            "none",
            "fade",
            "slide-left",
            "slide-right",
            "wipe-left",
            "wipe-right",
            "zoom",
            "dissolve",
        ] {
            let mut deck = DeckModel::new_default();
            deck.slides[0].transition = transition.to_string();
            let warnings = export_compatibility_warnings(
                "slide",
                "pptx",
                &serde_json::to_value(deck).expect("serialize deck"),
            );
            assert!(
                !warnings
                    .iter()
                    .any(|warning| warning.contains("transition")),
                "unexpected transition warning for {transition}: {warnings:?}"
            );
        }
        let mut animated_deck = DeckModel::new_default();
        animated_deck.slides[0].elements[0].entrance = "zoom".to_string();
        let animation_warnings = export_compatibility_warnings(
            "slide",
            "pptx",
            &serde_json::to_value(animated_deck).expect("serialize animated deck"),
        );
        assert!(!animation_warnings
            .iter()
            .any(|warning| warning.contains("entrance animation")));
    }

    #[test]
    fn reports_unsafe_pptx_hyperlinks() {
        let mut deck = DeckModel::new_default();
        deck.slides[0].elements[0].hyperlink = Some("javascript:alert(1)".to_string());
        let warnings = export_compatibility_warnings(
            "slide",
            "pptx",
            &serde_json::to_value(deck).expect("serialize deck"),
        );
        assert!(warnings.iter().any(|warning| warning.contains("hyperlink")));
    }
}
