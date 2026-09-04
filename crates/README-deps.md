# Redoc Crate Dependency Ledger

This document tracks external crate dependencies, their purposes, approximate binary size costs, and rejected alternatives.

| Crate / Dep | Crate Used In | Purpose | Size Cost | Alternatives Rejected |
| ----------- | ------------- | ------- | --------- | --------------------- |
| `serde`, `serde_json` | `core`, `file-io`, `doc-engine`, etc. | Data serialization / deserialization for JSON & models | ~300 KB | `bincode` (less debuggable), `cbor` (unnecessary overhead) |
| `zip` | `core`, `file-io`, `export` | Reading/writing `.redoc` ZIP container files and OOXML packages | ~250 KB | `async-zip` (unneeded async complexity for local disk sync) |
| `quick-xml` | `file-io`, `export` | High performance XML parser/writer for OOXML / DOCX / PPTX | ~180 KB | `xml-rs` (slower, memory intensive) |
| `printpdf`, `image` | `export` | Native PDF generation with embedded PNG/JPEG image support | ~1.2 MB | Headless chromium (heavy 150MB+ binary) |
| `docx-rs` | `export` | DOCX read/write for document export and import | ~400 KB | Hand-built OOXML XML only |
| `rust_xlsxwriter` | `export` | XLSX export writer | ~300 KB | `umya-spreadsheet` (heavier API surface) |
| `calamine` | `export` | XLSX import / spreadsheet parsing | ~200 KB | Manual OOXML parsing |
| `unicode-segmentation` | `export`, `doc-engine` | Grapheme-aware text segmentation for layout | ~30 KB | Byte-index string slicing |
| `thiserror` | Workspace-wide | Idiomatic typed error definition | ~20 KB | Hand-rolled `Display` / `Error` boilerplate |
| `tracing`, `tracing-appender` | `core` | Structured logging with size-bounded rotation (5 MiB active file + 4 rotated backups via non-blocking `SizeRollingWriter`) | ~120 KB | `log` + `env_logger` (lacks log file rotation) |
| `parking_lot` | `core` | High performance sync primitives (`Mutex`, `RwLock`) | ~40 KB | `std::sync` (higher contention overhead on Windows) |
| `uuid` | `core`, `file-io` | UUIDv7 document ID generation | ~30 KB | Custom random strings (lack timestamp sorting) |
| `sha2` | `file-io` | Content-addressed asset deduplication (SHA-256) | ~40 KB | `md-5` (cryptographically broken) |
| `specta`, `tauri-specta` | `desktop/src-tauri` | Type-safe end-to-end TS bindings generation | ~150 KB | Hand-written TypeScript interfaces (prone to drift) |
| `proptest` | `formula` (dev) | Property-based testing for formula parser & evaluation engine | Dev-only | Manual unit test cases alone |

## Removed / not in use

- **`tiny-skia`**, **`fontdb`** — previously listed in early design docs; not present in any current `Cargo.toml`. PDF/text layout uses `printpdf` and `unicode-segmentation` instead.

## Version notes

### `zip` (unified at 2.4)

| Crate | `Cargo.toml` pin |
| ----- | ---------------- |
| `redoc-core` | `zip = "2.4"` |
| `redoc-file-io` | `zip = "2.4"` |
| `redoc-export` | `zip = "2.4"` |

All workspace members use the single `zip` 2.4 line (deflate-only features) for `.redoc` containers and OOXML packages.
