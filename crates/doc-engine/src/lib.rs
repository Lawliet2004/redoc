pub mod schema;
pub mod search;
pub mod word_count;

pub use schema::{
    catalog, estimate_page_count, normalize_mark_type, prune_doc, schema_json, validate_doc,
    SchemaCatalog, MARK_TYPES, NODE_TYPES, SCHEMA_VERSION,
};
pub use search::{search_doc, SearchMatch};
pub use word_count::{compute_word_count, DocWordCount};
