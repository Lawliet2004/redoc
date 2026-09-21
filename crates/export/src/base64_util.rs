/// Shared base64 helpers for export pipelines (PDF, PPTX, DOCX).
use base64::{engine::general_purpose::STANDARD, Engine};

pub fn decode_base64(value: &str) -> Option<Vec<u8>> {
    let bytes: Vec<u8> = value.bytes().filter(|b| !b.is_ascii_whitespace()).collect();
    STANDARD.decode(bytes).ok()
}

pub fn encode_base64(bytes: &[u8]) -> String {
    STANDARD.encode(bytes)
}

pub fn data_uri_bytes(value: &str) -> Option<Vec<u8>> {
    let (_, encoded) = value.split_once(',')?;
    value.starts_with("data:").then(|| decode_base64(encoded))?
}
