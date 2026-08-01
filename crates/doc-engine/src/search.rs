use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SearchMatch {
    pub text: String,
    pub index: usize,
    pub line_number: usize,
}

pub fn search_doc(
    doc_json: &serde_json::Value,
    query: &str,
    case_sensitive: bool,
) -> Vec<SearchMatch> {
    if query.is_empty() {
        return Vec::new();
    }
    let mut matches = Vec::new();
    let q = if case_sensitive {
        query.to_string()
    } else {
        query.to_lowercase()
    };

    fn collect_inline_text(node: &serde_json::Value, line: &mut String) {
        let node_type = node.get("type").and_then(|t| t.as_str()).unwrap_or("");
        if node_type == "text" {
            if let Some(text) = node.get("text").and_then(|t| t.as_str()) {
                line.push_str(text);
            }
        } else if node_type == "hard_break" {
            line.push('\n');
        }
        if let Some(content) = node.get("content").and_then(|c| c.as_array()) {
            for child in content {
                collect_inline_text(child, line);
            }
        }
    }

    fn collect_lines(node: &serde_json::Value, lines: &mut Vec<String>) {
        let node_type = node.get("type").and_then(|t| t.as_str()).unwrap_or("");
        if node_type == "paragraph" || node_type == "heading" {
            let mut line = String::new();
            collect_inline_text(node, &mut line);
            lines.push(line);
        }
        if let Some(content) = node.get("content").and_then(|c| c.as_array()) {
            for child in content {
                collect_lines(child, lines);
            }
        }
    }

    let mut lines = Vec::new();
    collect_lines(doc_json, &mut lines);

    for (line_idx, text) in lines.iter().enumerate() {
        let mut target = String::new();
        let mut map = Vec::with_capacity(text.len());
        
        for (i, c) in text.char_indices() {
            let len = c.len_utf8();
            if case_sensitive {
                let mut buf = [0; 4];
                let s = c.encode_utf8(&mut buf);
                target.push_str(s);
                for _ in 0..s.len() {
                    map.push((i, i + len));
                }
            } else {
                for mc in c.to_lowercase() {
                    let mut buf = [0; 4];
                    let s = mc.encode_utf8(&mut buf);
                    target.push_str(s);
                    for _ in 0..s.len() {
                        map.push((i, i + len));
                    }
                }
            }
        }

        let mut start = 0;
        while start < target.len() {
            if let Some(idx) = target[start..].find(&q) {
                let actual_idx = start + idx;
                let end_idx = actual_idx + q.len();

                if actual_idx >= map.len() || end_idx > map.len() || end_idx == 0 {
                    start = actual_idx + 1;
                    continue;
                }

                let orig_start = map[actual_idx].0;
                let orig_end = map[end_idx - 1].1;
                
                matches.push(SearchMatch {
                    text: text[orig_start..orig_end].to_string(),
                    index: orig_start,
                    line_number: line_idx + 1,
                });
                
                start = end_idx;
            } else {
                break;
            }
        }
    }

    matches
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_non_ascii_utf8() {
        let doc = serde_json::json!({
            "type": "doc",
            "content": [
                {
                    "type": "paragraph",
                    "content": [
                        { "type": "text", "text": "Héllo wörld" },
                        { "type": "text", "text": "Testing UTF-8: Café, résumé, naïve" }
                    ]
                }
            ]
        });

        let matches = search_doc(&doc, "wörld", true);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].text, "wörld");
        assert_eq!(matches[0].line_number, 1);

        let matches_ci = search_doc(&doc, "héllo", false);
        assert_eq!(matches_ci.len(), 1);
        assert_eq!(matches_ci[0].text, "Héllo");

        let matches_caf = search_doc(&doc, "CAFÉ", false);
        assert_eq!(matches_caf.len(), 1);
        assert_eq!(matches_caf[0].text, "Café");
    }

    #[test]
    fn test_search_complex_utf8() {
        let doc = serde_json::json!({
            "type": "doc",
            "content": [
                {
                    "type": "paragraph",
                    "content": [
                        { "type": "text", "text": "Groß and İSTANBUL" }
                    ]
                }
            ]
        });

        // Note: Rust's char::to_lowercase() for 'ß' is just 'ß', it doesn't become 'ss'.
        let matches_ss = search_doc(&doc, "ß", false);
        assert_eq!(matches_ss.len(), 1);
        assert_eq!(matches_ss[0].text, "ß");

        // "İ" becomes "i\u{307}", searching for "istanbul" (with standard i) should match if we search for exactly the lowered string,
        // actually let's just search for the lowercased version of the word.
        let q = "i\u{307}stanbul";
        let matches_ist = search_doc(&doc, q, false);
        assert_eq!(matches_ist.len(), 1);
        assert_eq!(matches_ist[0].text, "İSTANBUL");
    }
}
