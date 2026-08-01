# Redoc manual QA checklist

Run this checklist against a fresh profile and an existing profile before release. Unchecked boxes are **manual** steps — they are not implied to have automated coverage.

## Shared shell

- [ ] Home screen creates a document, spreadsheet, and presentation.
- [ ] `Ctrl+Alt+1/2/3` switches editor modes and `Ctrl+K` opens the command palette.
- [ ] Light, dark, and system themes render with readable contrast.
- [ ] With OS high-contrast / `prefers-contrast: more`, chrome tokens remain readable and toolbar focus rings are visible.
- [ ] Toolbar rows support keyboard focus: Tab into a toolbar, then ArrowLeft/Right among buttons.
- [ ] Settings and pinned recents survive application restart.
- [ ] Settings → **Check for updates** can be toggled; with unsigned/dev builds the app does not crash if the updater is unavailable.
- [ ] **F1** opens the keyboard-shortcuts dialog; **F4** repeats the last formatting command.
- [ ] Drag-and-drop a `.redoc` file onto the window opens it in the correct mode.
- [ ] **Save As** writes a new file and updates the window title.
- [ ] Switching modes with unsaved edits prompts before discarding.
- [ ] Recovery dialog appears after crash/autosave; **Discard all** clears snapshots without corrupting open work.

## Document

- [ ] Paragraphs, headings (H1–H6), bold, italic, underline, strike, blockquote, code, and lists round-trip through `.redoc`.
- [ ] Undo/redo, find/replace, word count, and title editing work without losing content.
- [ ] Table merge/split and column resize behave as expected.
- [ ] Page setup (margins/orientation) persists after save/reopen.
- [ ] Print preview renders before printing.
- [ ] Image drop and resize handle update the document.
- [ ] Pasting from the web is sanitized (no script tags / hostile markup).
- [ ] Save, reopen, PDF export, and DOCX export produce readable files.
- [ ] DOCX import preserves basic lists, links, and highlight/color where supported.

## Spreadsheet

- [ ] Cell edits and formulas recalculate and persist.
- [ ] Multiple sheets can be selected, renamed, and saved.
- [ ] AutoFilter: enable → column chevrons → uncheck value hides rows → survives `.redoc` reload.
- [ ] Sort… dialog: multi-key stable order; undo restores prior state.
- [ ] Merged cells display and export correctly.
- [ ] Freeze panes, insert/delete rows/cols, and find/replace work from toolbar or context menu.
- [ ] Named ranges, number formats, validation lists, and hyperlinks persist.
- [ ] Chart sidebar binds to a range and renders bar/line/pie.
- [ ] CSV import and export round-trip quoted values and formulas-as-text.
- [ ] Keyboard navigation and selection remain usable at high zoom.
- [ ] Sheet print (selection / whole sheet) produces expected output.

## Presentation

- [ ] Slides can be added, selected, moved, and reordered.
- [ ] Text, shapes (rect/ellipse/star/arrow/etc.), images, tables, and charts survive save/reopen.
- [ ] 8-handle resize and zoom behave predictably.
- [ ] Layout masters apply placeholder elements.
- [ ] Align, distribute, group, and ungroup work on multi-selection.
- [ ] Presenter window opens with current/next slide and speaker notes.
- [ ] Entrance fade animations step through in presenter view.
- [ ] Handout print (1/2/4/6 per page, optional notes) layouts correctly.
- [ ] PDF/PPTX export opens or reports a clear error.

## Packaging and OS integration

- [ ] Double-click / “Open with” a `.redoc` document (doc, sheet, and slide variants) launches Redoc in the matching editor mode.
- [ ] Opening a second `.redoc` while Redoc is running loads that file into the correct mode (where the OS delivers the open event).
- [ ] Release installer artifacts are ≤ 55 MB (`pnpm quality:check` after `tauri build`, or release CI).
- [ ] `pnpm quality:check` passes after `pnpm build` (frontend gzip budget, generated bindings, config gates).
- [ ] Opening `.pptx` shows a friendly “not yet supported” message (no silent failure).

## Recovery and failure handling

- [ ] Edit a document, wait for autosave, terminate the app, and verify recovery on next launch.
- [ ] Cancel native open/save dialogs without changing the current document.
- [ ] Corrupt or unsupported files show an error without crashing or overwriting user data.

## Automated tests (run via CI / `pnpm test` / `cargo test`)

These have unit or integration coverage in the repo today:

- [x] Formula parser and evaluator property tests (`crates/formula`, proptest).
- [x] Formula eval edge cases + dep_graph cycle detection (`crates/formula/src/eval.rs`, `dep_graph.rs`).
- [x] Container round-trip tests for doc, sheet, and slide bodies (`crates/file-io/tests/roundtrip.rs`).
- [x] Corrupt ZIP returns typed error, not panic (`corrupt_zip_returns_error_not_panic`).
- [x] Missing asset reference handled on load (`missing_asset` test).
- [x] Schema migration v0 → current (`crates/file-io/src/migrations.rs`).
- [x] Recovery `clear_all_snapshots` (`crates/core/src/recovery.rs`).
- [x] Base64 encoding/decoding and TSV parse/serialize (`packages/utils/src/index.test.ts`).
- [x] Sheet undo/history manager (`packages/sheet-editor/src/sheetHistory.test.ts`).
- [x] Sheet workbook model builder (`packages/sheet-editor/src/sheetModel.test.ts`).
- [x] Doc HTML paste sanitizer (`packages/doc-editor/src/pasteSanitizer.test.ts`).
- [x] Shortcut registry / buildMenus (`packages/editor-common/src/ShortcutRegistry.test.ts`).
- [x] Slide deck normalize/toDeck round-trip (`packages/slide-editor/src/deckNormalize.test.ts`).
- [x] Slide geometry, align, group helpers (`packages/slide-editor/src/geometry.test.ts`).
- [x] Shell a11y axe scans for HomeScreen, Dialog, CommandPalette, FindBar (`tests/a11y/shell.a11y.test.tsx`).
- [x] Weekly nightly formula fuzz workflow (`.github/workflows/nightly-fuzz.yml`).
