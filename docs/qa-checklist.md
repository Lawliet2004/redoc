# Redoc manual QA checklist

Run this checklist against a fresh profile and an existing profile before release.

## Shared shell

- [x] Home screen creates a document, spreadsheet, and presentation.
- [x] `Ctrl+Alt+1/2/3` switches editor modes and `Ctrl+K` opens the command palette.
- [x] Light, dark, and system themes render with readable contrast.
- [x] With OS high-contrast / `prefers-contrast: more`, chrome tokens remain readable and toolbar focus rings are visible.
- [x] Toolbar rows support keyboard focus: Tab into a toolbar, then ArrowLeft/Right among buttons.
- [x] Settings and pinned recents survive application restart.
- [x] Settings → **Check for updates** can be toggled; with unsigned/dev builds the app does not crash if the updater is unavailable.

## Document

- [x] Paragraphs, headings, bold, italic, underline, strike, blockquote, code, and lists round-trip through `.redoc`.
- [x] Undo/redo, find, word count, and title editing work without losing content.
- [x] Save, reopen, PDF export, and DOCX export produce readable files.

## Spreadsheet

- [x] Cell edits and formulas recalculate and persist.
- [x] Multiple sheets can be selected and saved.
- [x] CSV import and export round-trip quoted values and formulas-as-text.
- [x] Keyboard navigation and selection remain usable at high zoom.

## Presentation

- [x] Slides can be added, selected, moved, and reordered where supported.
- [x] Text, shapes, notes, and theme colors survive save/reopen.
- [x] PDF/PPTX export opens or reports a clear error.

## Packaging and OS integration

- [x] Double-click / “Open with” a `.redoc` document (doc, sheet, and slide variants) launches Redoc in the matching editor mode.
- [x] Opening a second `.redoc` while Redoc is running loads that file into the correct mode (where the OS delivers the open event).
- [x] Release installer artifacts are ≤ 55 MB (`pnpm quality:check` after `tauri build`, or release CI).
- [x] `pnpm quality:check` encodes the warm recalc ≤ 16 ms contract (release CI runs the full bench via `REDOC_RELEASE_CI=1`; locally use `pnpm performance:check` or `REDOC_PERF_GATE=1`).

## Recovery and failure handling

- [x] Edit a document, wait for autosave, terminate the app, and verify recovery on next launch.
- [x] Cancel native open/save dialogs without changing the current document.
- [x] Corrupt or unsupported files show an error without crashing or overwriting user data.

## Phase 7 tests

- [x] Formula parser and evaluator fuzz testing passes without panics.
- [x] Golden file round-trip tests for Doc, Sheet, Slide succeed.
- [x] Corrupt zip files are handled gracefully without panics.
- [x] Base64 encoding/decoding and TSV parsing/serialization tested.
- [x] History manager push, undo, and redo tested in sheet editor.
- [x] Element bounds calculation and hit testing tested in slide editor.
- [x] CI hardening ensures cargo test --workspace and cargo clippy execute properly before JS builds.
