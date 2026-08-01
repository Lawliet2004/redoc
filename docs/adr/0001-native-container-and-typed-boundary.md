# ADR 0001: Native container and typed command boundary

## Decision

Redoc uses a ZIP container with JSON metadata/body files for `.redoc` documents. The Tauri command boundary is the only frontend/backend integration point; filesystem operations and domain validation remain in Rust.

## Rationale

JSON keeps files inspectable and migration-friendly while ZIP compression keeps ordinary documents small. A single boundary prevents editors from silently diverging in serialization and makes autosave/recovery testable without a webview.
