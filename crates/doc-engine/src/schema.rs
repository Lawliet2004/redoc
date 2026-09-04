//! Authoritative document schema catalog used by the editor and exporters.
//! Frontend ProseMirror schema mirrors these mark/node names.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub const SCHEMA_VERSION: u32 = 1;

pub const MARK_TYPES: &[&str] = &[
    "bold",
    "italic",
    "underline",
    "strike",
    "link",
    "bookmark",
    "color",
    "highlight",
    "fontFamily",
    "fontSize",
    "superscript",
    "subscript",
    "trackInsert",
    "trackDelete",
];

pub const NODE_TYPES: &[&str] = &[
    "doc",
    "paragraph",
    "heading",
    "blockquote",
    "code_block",
    "horizontal_rule",
    "image",
    "bullet_list",
    "ordered_list",
    "list_item",
    "table",
    "table_row",
    "table_cell",
    "table_header",
    "page_break",
    "section_break",
    "hard_break",
    "field",
    "footnote_ref",
    "text",
];

/// Aliases accepted when reading legacy / ProseMirror-basic mark names.
pub fn normalize_mark_type(name: &str) -> &str {
    match name {
        "strong" => "bold",
        "em" => "italic",
        other => other,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SchemaCatalog {
    pub version: u32,
    pub marks: Vec<String>,
    pub nodes: Vec<String>,
}

pub fn catalog() -> SchemaCatalog {
    SchemaCatalog {
        version: SCHEMA_VERSION,
        marks: MARK_TYPES.iter().map(|s| (*s).to_string()).collect(),
        nodes: NODE_TYPES.iter().map(|s| (*s).to_string()).collect(),
    }
}

pub fn schema_json() -> Value {
    json!({
        "version": SCHEMA_VERSION,
        "marks": MARK_TYPES,
        "nodes": NODE_TYPES,
        "markAliases": {
            "strong": "bold",
            "em": "italic"
        }
    })
}

/// Estimate page count based on explicit page_break nodes, paragraph/heading lines, and text length heuristics.
pub fn estimate_page_count(doc: &Value) -> u32 {
    let mut pages = 1u32;
    let mut words = 0u32;
    let mut lines = 0u32;

    fn walk(node: &Value, pages: &mut u32, words: &mut u32, lines: &mut u32) {
        let node_type = node.get("type").and_then(|t| t.as_str()).unwrap_or("");

        let section_break_counts_page = node_type == "section_break"
            && node
                .get("attrs")
                .and_then(|attrs| attrs.get("pageSetup"))
                .and_then(|setup| setup.get("breakType"))
                .and_then(|value| value.as_str())
                .is_none_or(|break_type| break_type != "continuous" && break_type != "nextColumn");
        if node_type == "page_break" || section_break_counts_page {
            *pages += 1;
            *words = 0;
            *lines = 0;
        }

        if node_type == "paragraph" || node_type == "heading" {
            *lines += 1;
        }

        if node_type == "text" {
            if let Some(text) = node.get("text").and_then(|t| t.as_str()) {
                *words += text.split_whitespace().count() as u32;
            }
        }

        if *words >= 500 || *lines >= 45 {
            *pages += 1;
            *words = 0;
            *lines = 0;
        }

        if let Some(children) = node.get("content").and_then(|c| c.as_array()) {
            for child in children {
                walk(child, pages, words, lines);
            }
        }
    }

    walk(doc, &mut pages, &mut words, &mut lines);
    pages
}

/// Validates document AST structure against authoritative schema rules.
/// Returns Ok(()) if valid, or Err(Vec<String>) containing validation error descriptions.
pub fn validate_doc(doc_json: &Value) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();

    if !doc_json.is_object() {
        return Err(vec!["Document root must be a JSON object".to_string()]);
    }

    fn walk(node: &Value, path: &str, errors: &mut Vec<String>) {
        let Some(obj) = node.as_object() else {
            errors.push(format!("Node at path '{path}' must be a JSON object"));
            return;
        };

        let node_type = match obj.get("type").and_then(|t| t.as_str()) {
            Some(t) => t,
            None => {
                errors.push(format!(
                    "Node at path '{path}' is missing required 'type' field"
                ));
                return;
            }
        };

        if !NODE_TYPES.contains(&node_type) {
            errors.push(format!("Invalid node type '{node_type}' at path '{path}'"));
        }

        // Validate node content structure
        if node_type == "doc" {
            match obj.get("content") {
                Some(c) if c.is_array() => {}
                Some(_) => errors.push(format!(
                    "Root 'doc' node at path '{path}' has non-array 'content'"
                )),
                None => errors.push(format!(
                    "Root 'doc' node at path '{path}' is missing required 'content' array"
                )),
            }
        }

        if let Some(content_val) = obj.get("content") {
            if let Some(children) = content_val.as_array() {
                for (idx, child) in children.iter().enumerate() {
                    let child_path = format!("{path}.content[{idx}]");

                    if let Some(child_obj) = child.as_object() {
                        if let Some(child_type) = child_obj.get("type").and_then(|t| t.as_str()) {
                            if node_type == "table" && child_type != "table_row" {
                                errors.push(format!("Node of type 'table' at path '{path}' contains invalid child type '{child_type}' at '{child_path}'. Expected 'table_row'"));
                            } else if node_type == "table_row"
                                && child_type != "table_cell"
                                && child_type != "table_header"
                            {
                                errors.push(format!("Node of type 'table_row' at path '{path}' contains invalid child type '{child_type}' at '{child_path}'. Expected 'table_cell' or 'table_header'"));
                            } else if (node_type == "bullet_list" || node_type == "ordered_list")
                                && child_type != "list_item"
                            {
                                errors.push(format!("Node of type '{node_type}' at path '{path}' contains invalid child type '{child_type}' at '{child_path}'. Expected 'list_item'"));
                            }
                        }
                    }

                    walk(child, &child_path, errors);
                }
            } else if node_type != "doc" {
                errors.push(format!(
                    "Node of type '{node_type}' at path '{path}' has non-array 'content'"
                ));
            }
        }

        if node_type == "heading" {
            let valid_level = obj
                .get("attrs")
                .and_then(|a| a.as_object())
                .and_then(|a| a.get("level"))
                .and_then(|l| l.as_u64())
                .is_some_and(|l| (1..=6).contains(&l));

            if !valid_level {
                errors.push(format!("Node of type 'heading' at path '{path}' must have an 'attrs.level' integer between 1 and 6"));
            }
        }

        // Validate marks
        if let Some(marks_val) = obj.get("marks") {
            if let Some(marks_arr) = marks_val.as_array() {
                for (idx, mark) in marks_arr.iter().enumerate() {
                    let mark_path = format!("{path}.marks[{idx}]");
                    if let Some(mark_obj) = mark.as_object() {
                        if let Some(mark_type) = mark_obj.get("type").and_then(|t| t.as_str()) {
                            let normalized = normalize_mark_type(mark_type);
                            if !MARK_TYPES.contains(&normalized) {
                                errors.push(format!(
                                    "Invalid mark type '{mark_type}' at path '{mark_path}'"
                                ));
                            }
                        } else {
                            errors.push(format!(
                                "Mark object at path '{mark_path}' is missing 'type' field"
                            ));
                        }
                    } else {
                        errors.push(format!(
                            "Mark entry at path '{mark_path}' must be a JSON object"
                        ));
                    }
                }
            } else {
                errors.push(format!(
                    "Node of type '{node_type}' at path '{path}' has non-array 'marks'"
                ));
            }
        }
    }

    walk(doc_json, "root", &mut errors);

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Remove invalid node types and marks so downstream consumers receive a safe document.
pub fn prune_doc(doc_json: &Value) -> Value {
    fn prune_content(children: &[Value]) -> Vec<Value> {
        children.iter().filter_map(prune_node).collect()
    }

    fn prune_marks(marks: &[Value]) -> Vec<Value> {
        marks
            .iter()
            .filter(|mark| {
                mark.as_object()
                    .and_then(|m| m.get("type"))
                    .and_then(|t| t.as_str())
                    .map(|t| MARK_TYPES.contains(&normalize_mark_type(t)))
                    .unwrap_or(false)
            })
            .cloned()
            .collect()
    }

    fn prune_node(node: &Value) -> Option<Value> {
        let obj = node.as_object()?;
        let node_type = obj.get("type").and_then(|t| t.as_str())?;
        if !NODE_TYPES.contains(&node_type) {
            return None;
        }

        let mut out = obj.clone();
        if node_type == "text" {
            if let Some(marks) = obj.get("marks").and_then(|m| m.as_array()) {
                let cleaned = prune_marks(marks);
                if cleaned.is_empty() {
                    out.remove("marks");
                } else {
                    out.insert("marks".to_string(), Value::Array(cleaned));
                }
            }
            return Some(Value::Object(out));
        }

        if let Some(content) = obj.get("content").and_then(|c| c.as_array()) {
            out.insert("content".to_string(), Value::Array(prune_content(content)));
        }

        Some(Value::Object(out))
    }

    if let Some(root) = doc_json.as_object() {
        let mut out = root.clone();
        if let Some(content) = root.get("content").and_then(|c| c.as_array()) {
            out.insert("content".to_string(), Value::Array(prune_content(content)));
        }
        Value::Object(out)
    } else {
        doc_json.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_legacy_marks() {
        assert_eq!(normalize_mark_type("strong"), "bold");
        assert_eq!(normalize_mark_type("em"), "italic");
        assert_eq!(normalize_mark_type("underline"), "underline");
    }

    #[test]
    fn table_header_is_a_known_node_type() {
        assert!(NODE_TYPES.contains(&"table_header"));
        // A header-row table must validate and survive pruning.
        let doc = json!({
            "type": "doc",
            "content": [{
                "type": "table",
                "content": [{
                    "type": "table_row",
                    "content": [
                        { "type": "table_header", "content": [{ "type": "paragraph", "content": [{ "type": "text", "text": "H1" }] }] },
                        { "type": "table_cell", "content": [{ "type": "paragraph", "content": [{ "type": "text", "text": "D1" }] }] }
                    ]
                }]
            }]
        });
        assert!(validate_doc(&doc).is_ok(), "header-row table must validate");
        let pruned = prune_doc(&doc);
        let cell_types: Vec<&str> = pruned["content"][0]["content"][0]["content"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["type"].as_str().unwrap())
            .collect();
        assert_eq!(cell_types, vec!["table_header", "table_cell"]);
    }

    #[test]
    fn page_count_counts_breaks() {
        let doc = json!({
            "type": "doc",
            "content": [
                {"type": "paragraph", "content": [{"type": "text", "text": "a"}]},
                {"type": "page_break"},
                {"type": "paragraph", "content": [{"type": "text", "text": "b"}]}
            ]
        });
        assert_eq!(estimate_page_count(&doc), 2);
    }

    #[test]
    fn page_count_counts_section_breaks() {
        let doc = json!({
            "type": "doc",
            "content": [
                {"type": "paragraph", "content": [{"type": "text", "text": "a"}]},
                {"type": "section_break", "attrs": {"pageSetup": {"orientation": "landscape"}}},
                {"type": "paragraph", "content": [{"type": "text", "text": "b"}]}
            ]
        });
        assert_eq!(estimate_page_count(&doc), 2);
    }

    #[test]
    fn continuous_section_break_stays_on_current_page() {
        let doc = json!({
            "type": "doc",
            "content": [
                {"type": "paragraph", "content": [{"type": "text", "text": "a"}]},
                {"type": "section_break", "attrs": {"pageSetup": {"breakType": "continuous"}}},
                {"type": "paragraph", "content": [{"type": "text", "text": "b"}]}
            ]
        });
        assert_eq!(estimate_page_count(&doc), 1);
    }

    #[test]
    fn schema_json_includes_new_marks() {
        let s = schema_json();
        let marks = s["marks"].as_array().unwrap();
        assert!(marks.iter().any(|m| m.as_str() == Some("fontFamily")));
        assert!(marks.iter().any(|m| m.as_str() == Some("superscript")));
        assert!(marks.iter().any(|m| m.as_str() == Some("bookmark")));
        let nodes = s["nodes"].as_array().unwrap();
        assert!(nodes.iter().any(|n| n.as_str() == Some("page_break")));
        assert!(nodes.iter().any(|n| n.as_str() == Some("section_break")));
        assert!(nodes.iter().any(|n| n.as_str() == Some("field")));
    }

    #[test]
    fn test_validate_doc_structure() {
        let valid_doc = json!({
            "type": "doc",
            "content": [
                {
                    "type": "paragraph",
                    "content": [
                        {
                            "type": "text",
                            "text": "Hello world",
                            "marks": [{ "type": "bold" }, { "type": "strong" }]
                        }
                    ]
                }
            ]
        });
        assert!(validate_doc(&valid_doc).is_ok());

        let invalid_root = json!({
            "type": "not_a_doc"
        });
        assert!(validate_doc(&invalid_root).is_err());

        let missing_content = json!({
            "type": "doc"
        });
        assert!(validate_doc(&missing_content).is_err());

        let invalid_mark = json!({
            "type": "doc",
            "content": [
                {
                    "type": "paragraph",
                    "content": [
                        {
                            "type": "text",
                            "text": "Hello",
                            "marks": [{ "type": "invalid_mark_name" }]
                        }
                    ]
                }
            ]
        });
        let res = validate_doc(&invalid_mark);
        assert!(res.is_err());
        assert!(res
            .unwrap_err()
            .iter()
            .any(|e| e.contains("invalid_mark_name")));
    }

    #[test]
    fn test_structural_validation() {
        let invalid_table = json!({
            "type": "doc",
            "content": [
                {
                    "type": "table",
                    "content": [{"type": "paragraph"}]
                }
            ]
        });
        assert!(validate_doc(&invalid_table)
            .unwrap_err()
            .iter()
            .any(|e| e.contains("Expected 'table_row'")));

        let invalid_list = json!({
            "type": "doc",
            "content": [
                {
                    "type": "bullet_list",
                    "content": [{"type": "paragraph"}]
                }
            ]
        });
        assert!(validate_doc(&invalid_list)
            .unwrap_err()
            .iter()
            .any(|e| e.contains("Expected 'list_item'")));

        let invalid_heading = json!({
            "type": "doc",
            "content": [
                {
                    "type": "heading",
                    "attrs": { "level": 7 },
                    "content": [{"type": "text", "text": "H7"}]
                }
            ]
        });
        assert!(validate_doc(&invalid_heading)
            .unwrap_err()
            .iter()
            .any(|e| e.contains("integer between 1 and 6")));
    }

    #[test]
    fn hard_break_is_valid_node() {
        let doc = json!({
            "type": "doc",
            "content": [{
                "type": "paragraph",
                "content": [
                    { "type": "text", "text": "Line1" },
                    { "type": "hard_break" },
                    { "type": "text", "text": "Line2" }
                ]
            }]
        });
        assert!(validate_doc(&doc).is_ok());
    }

    /// Keeps `packages/doc-editor/src/schema-catalog.json` in sync with the
    /// authoritative Rust catalog. Run via `cargo test -p redoc-doc-engine`.
    #[test]
    fn exports_schema_catalog_json() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../packages/doc-editor/src/schema-catalog.json");
        let catalog =
            serde_json::to_string_pretty(&schema_json()).expect("serialize schema catalog");
        // Normalize to LF so the committed file is stable across platforms.
        let catalog = catalog.replace("\r\n", "\n");
        let existing = std::fs::read_to_string(&path);
        match existing {
            Ok(current) if current == catalog => {}
            _ => {
                std::fs::write(&path, &catalog)
                    .expect("write schema-catalog.json next to the editor schema");
            }
        }
        // The exported catalog must always contain every mark/node the
        // exporters and editor rely on.
        let value: Value = serde_json::from_str(&catalog).expect("catalog is valid JSON");
        assert_eq!(value["version"], SCHEMA_VERSION);
        for mark in MARK_TYPES {
            assert!(
                value["marks"].as_array().unwrap().iter().any(|m| m == mark),
                "catalog missing mark {mark}"
            );
        }
        for node in NODE_TYPES {
            assert!(
                value["nodes"].as_array().unwrap().iter().any(|n| n == node),
                "catalog missing node {node}"
            );
        }
    }
}
