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
        Some("doc") | Some("paragraph") | Some("heading") | Some("blockquote")
        | Some("bullet_list") | Some("ordered_list") | Some("list_item") | Some("table")
        | Some("table_row") | Some("table_cell") | Some("code_block") | Some("page_break")
        | Some("text") | Some("image") => {}
        Some("hard_break") => push_once(
            warnings,
            "DOCX: hard breaks are simplified to Word text-wrapping breaks.",
        ),
        Some("horizontal_rule") => push_once(
            warnings,
            "DOCX: horizontal rules are not emitted by the current writer exporter.",
        ),
        Some("table_of_contents") | Some("field") | Some("content_control") => push_once(
            warnings,
            "DOCX: advanced fields/content controls are not preserved.",
        ),
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
                    | "trackDelete",
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

fn xlsx_warnings(workbook: &WorkbookModel) -> Vec<String> {
    let mut warnings = BTreeSet::new();
    for sheet in &workbook.sheets {
        if !sheet.pivot_tables.is_empty() {
            push_once(
                &mut warnings,
                "XLSX: pivot summaries export as materialized cells; native pivot parts and slicers are not emitted.",
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
            "none" | "fade" | "slide-left" | "slide-right"
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
            if !matches!(element.entrance.as_str(), "none" | "fade") {
                push_once(
                    &mut warnings,
                    format!(
                        "PPTX: entrance animation '{}' is not emitted.",
                        element.entrance
                    ),
                );
            }
            match &element.kind {
                ElementKind::Table { .. } => push_once(
                    &mut warnings,
                    "PPTX: tables export as editable text-grid fallback shapes, not native table parts.",
                ),
                ElementKind::Chart { .. } => push_once(
                    &mut warnings,
                    "PPTX: charts export as editable summary shapes, not native chart parts.",
                ),
                ElementKind::Shape { shape_type, .. }
                    if !matches!(shape_type.as_str(), "rect" | "ellipse" | "line" | "arrow") =>
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
    use redoc_sheet_engine::{ChartModel, SheetData};
    use redoc_slide_engine::{ElementKind, SlideElement};

    #[test]
    fn reports_docx_unsupported_nodes_and_images() {
        let body = serde_json::json!({
            "type": "doc",
            "content": [
                { "type": "horizontal_rule" },
                { "type": "image", "attrs": { "src": "https://example.invalid/image.png" } }
            ]
        });
        let warnings = export_compatibility_warnings("doc", "docx", &body);
        assert!(warnings
            .iter()
            .any(|warning| warning.contains("horizontal rules")));
        assert!(warnings
            .iter()
            .any(|warning| warning.contains("images without")));
    }

    #[test]
    fn reports_xlsx_pivot_and_unknown_chart() {
        let mut sheet = SheetData::new("sheet-1", "Sheet1");
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
                value_field: 2,
                aggregation: "sum".to_string(),
                output_start_row: 5,
                output_start_col: 1,
                output_row_count: None,
            });
        let workbook = WorkbookModel {
            sheets: vec![sheet],
            active_sheet_index: 0,
            ..WorkbookModel::new_default()
        };
        let warnings = export_compatibility_warnings(
            "sheet",
            "xlsx",
            &serde_json::to_value(workbook).expect("serialize workbook"),
        );
        assert!(warnings
            .iter()
            .any(|warning| warning.contains("pivot summaries")));
        assert!(warnings.iter().any(|warning| warning.contains("radar")));
    }

    #[test]
    fn reports_pptx_fallbacks_and_unknown_animation() {
        let mut deck = DeckModel::new_default();
        deck.slides[0].transition = "zoom".to_string();
        deck.slides[0].elements.push(SlideElement {
            id: "shape-1".to_string(),
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
            rotation: 0.0,
            z_index: 1,
            entrance: "zoom".to_string(),
            kind: ElementKind::Chart {
                chart_type: "bar".to_string(),
                data: vec![1.0],
                labels: vec!["Value".to_string()],
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
            .any(|warning| warning.contains("charts export")));
    }
}
