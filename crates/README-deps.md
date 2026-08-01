# Redoc Crate Dependency Ledger

This document tracks all external crate dependencies, their purposes, approximate binary size costs, and rejected alternatives.

| Crate / Dep | Crate Used In | Purpose | Size Cost | Alternatives Rejected |
| ----------- | ------------- | ------- | --------- | --------------------- |
| `serde`, `serde_json` | `core`, `file-io`, `doc-engine`, etc. | Data serialization / deserialization for JSON & models | ~300 KB | `bincode` (less debuggable), `cbor` (unnecessary overhead) |
| `zip` | `file-io`, `export` | Reading/writing `.redoc` ZIP container files | ~250 KB | `async-zip` (unneeded async complexity for local disk sync) |
| `quick-xml` | `file-io`, `export` | High performance XML parser/writer for OOXML / DOCX / PPTX | ~180 KB | `xml-rs` (slower, memory intensive) |
| `printpdf`, `image` | `export` | Native PDF generation with embedded PNG/JPEG image support | ~1.2 MB | Headless chromium (heavy 150MB+ binary) |
| `tiny-skia` | `export` | Fast software 2D rasterizer for rendering slide SVG elements into PDF images | ~400 KB | `cairo`, `skia-safe` (requires heavy C++ build dependencies) |
| `fontdb` | `export` | System font discovery and matching for PDF layout engine | ~150 KB | Hardcoding system font paths (brittle across OSes) |
| `thiserror` | Workspace-wide | Idiomatic typed error definition | ~20 KB | Hand-rolled `Display` / `Error` boilerplate |
| `tracing`, `tracing-appender` | `core` | Structured logging and daily log file rotation | ~120 KB | `log` + `env_logger` (lacks log file rotation) |
| `parking_lot` | `core` | High performance sync primitives (`Mutex`, `RwLock`) | ~40 KB | `std::sync` (higher contention overhead on Windows) |
| `uuid` | `core`, `file-io` | UUIDv7 document ID generation | ~30 KB | Custom random strings (lack timestamp sorting) |
| `sha2` | `file-io` | Content-addressed asset deduplication (SHA-256) | ~40 KB | `md-5` (cryptographically broken) |
| `specta`, `tauri-specta` | `desktop/src-tauri` | Type-safe end-to-end TS bindings generation | ~150 KB | Hand-written TypeScript interfaces (prone to drift) |
| `proptest` | `formula` (dev) | Property-based testing for formula parser & evaluation engine | Dev-only | Manual unit test cases alone |
