use serde::{Deserialize, Serialize};
use specta::Type;
use unicode_segmentation::UnicodeSegmentation;

#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DocWordCount {
    pub words: usize,
    pub characters: usize,
    pub paragraphs: usize,
}

pub fn compute_word_count(doc_json: &serde_json::Value) -> DocWordCount {
    let mut words = 0;
    let mut characters = 0;
    let mut paragraphs = 0;

    fn traverse(node: &serde_json::Value, words: &mut usize, chars: &mut usize, paras: &mut usize) {
        if let Some(node_type) = node.get("type").and_then(|t| t.as_str()) {
            if node_type == "paragraph" || node_type == "heading" {
                *paras += 1;
            }
            if node_type == "text" {
                if let Some(text) = node.get("text").and_then(|t| t.as_str()) {
                    *chars += text.chars().count();
                    *words += text.unicode_words().count();
                }
            }
        }

        if let Some(content) = node.get("content").and_then(|c| c.as_array()) {
            for child in content {
                traverse(child, words, chars, paras);
            }
        }
    }

    traverse(doc_json, &mut words, &mut characters, &mut paragraphs);

    DocWordCount {
        words,
        characters,
        paragraphs,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_word_count_standard() {
        let doc = json!({
            "type": "doc",
            "content": [
                {
                    "type": "paragraph",
                    "content": [
                        {
                            "type": "text",
                            "text": "Hello world!"
                        }
                    ]
                }
            ]
        });
        let count = compute_word_count(&doc);
        assert_eq!(count.words, 2);
        assert_eq!(count.paragraphs, 1);
        assert_eq!(count.characters, 12);
    }

    #[test]
    fn test_word_count_cjk() {
        let doc = json!({
            "type": "doc",
            "content": [
                {
                    "type": "paragraph",
                    "content": [
                        {
                            "type": "text",
                            "text": "你好世界"
                        }
                    ]
                }
            ]
        });
        let count = compute_word_count(&doc);
        // "你好世界" is 4 characters, 4 words with unicode_words
        assert_eq!(count.words, 4);
        assert_eq!(count.characters, 4);
    }

    #[test]
    fn test_word_count_em_dash() {
        let doc = json!({
            "type": "doc",
            "content": [
                {
                    "type": "paragraph",
                    "content": [
                        {
                            "type": "text",
                            "text": "word1—word2"
                        }
                    ]
                }
            ]
        });
        let count = compute_word_count(&doc);
        // "word1—word2" should be 2 words
        assert_eq!(count.words, 2);
    }
}
