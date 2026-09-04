# Redoc manual QA checklist

Run this checklist against a fresh profile and an existing profile before release. Unchecked boxes are **manual** steps — they are not implied to have automated coverage.

## Shared shell

- [ ] Home screen creates a document, spreadsheet, and presentation.
- [ ] `Ctrl+Alt+1/2/3` switches editor modes and `Ctrl+K` opens the command palette.
- [ ] `Ctrl+=` zooms in, `Ctrl+-` zooms out, and `Ctrl+0` resets zoom from any editor.
- [ ] Settings → author profile (display name / email / color) saves; new comments and tracked changes in all three editors attribute to that name.
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
- [ ] Page setup (paper size, margins, orientation, 1–4 text columns, headers/footers, and page fields) persists after save/reopen and DOCX round-trip; inserted section breaks retain their per-section properties.
- [ ] Writer Print Preview opens with the active paper size, orientation, per-edge margins, and header/footer fields; changing a preset or printing emits matching `@page` CSS, while custom margins remain visibly selected and bounded page placeholders resolve.
- [ ] Writer Insert → Field and the toolbar insert PAGE/NUMPAGES fields; field values refresh after layout changes, remain visible after save/reopen and DOCX round-trip, and PDF export shows the cached value or bounded placeholder.
- [x] Page setup metadata normalization is covered by pure tests (`packages/doc-editor/src/pageSetup.test.ts`).
- [ ] Print preview renders before printing.
- [ ] Image drop and resize handle update the document.
- [ ] Pasting from the web is sanitized (no script tags / hostile markup).
- [ ] Review sidebar lists tracked changes with individual Accept/Reject controls, and the all-changes controls resolve the remaining marks.
- [ ] Review sidebar shows a capped before/after comparison and token diff without making large documents unresponsive; when multiple Writer tabs are open, selecting another tab compares against that document and Reset returns to the opened baseline.
- [ ] Save, reopen, PDF export, and DOCX export produce readable files.
- [ ] DOCX import preserves basic lists, links, and highlight/color where supported.
- [ ] Writer paragraph alignment, indentation, line height, and before/after spacing survive DOCX export/import without malformed values.

## Spreadsheet

- [ ] Cell edits and formulas recalculate and persist.
- [ ] Cell borders: apply All/Outer/Top/Bottom/None presets with each line style and color over a multi-cell selection; verify render (incl. merged cells), `.redoc` reload, and XLSX round-trip.
- [ ] Multiple sheets can be selected, renamed, and saved.
- [ ] AutoFilter: enable → column chevrons → uncheck value hides rows → survives `.redoc` reload.
- [ ] Sort… dialog: multi-key stable order; undo restores prior state.
- [ ] Merged cells display and export correctly.
- [ ] Freeze panes, insert/delete rows/cols, and find/replace work from toolbar or context menu.
- [ ] Named ranges, number formats, literal and formula-backed validation lists (including bounded cross-sheet sources), and hyperlinks persist; Properties can apply/clear a cell hyperlink and Ctrl/Cmd-click opens safe external links or navigates bounded internal `Sheet!A1` targets.
- [ ] XLSX export rejects oversized, control-character, unsafe-protocol, and malformed internal hyperlink targets with a user-safe error.
- [x] Financial, conditional aggregate, date/time, and bounded statistical formulas (`PMT`, `PV`, `FV`, `NPV`, `IRR`, `SUMPRODUCT`, `MAXIFS`, `MINIFS`, `TIME`, `HOUR`, `MINUTE`, `SECOND`, `WEEKDAY`, `NETWORKDAYS`, `DAYS`, `EDATE`, `WORKDAY`, `RANK.EQ`, `QUARTILE.INC`, `CORREL`, `SUMSQ`) recalculate with valid inputs and show a safe formula error for invalid inputs, dates, array shapes, ties, zero variance, or non-finite arithmetic; dotted statistical aliases (`STDEV.P`, `VAR.S`, `PERCENTILE.INC`) and bounded `LET` bindings parse and evaluate.
- [x] Array formulas (`ROWS`, `COLUMNS`, `TRANSPOSE`, `SEQUENCE`, `RANDARRAY`, `HSTACK`, `VSTACK`, `TAKE`, `DROP`, `CHOOSECOLS`, `CHOOSEROWS`, `FILTER`, `UNIQUE`, `SORT`, `SORTBY`, `TOCOL`, `TOROW`, `WRAPROWS`, and `WRAPCOLS`) return validated dimensions/results and reject malformed or oversized arrays safely.
- [x] Everyday compatibility formulas (`CONCATENATE`, `COUNTBLANK`, and `XOR`) evaluate through the shared catalog and return bounded values/errors.
- [x] Dynamic-array formulas remain bounded when used inside other formulas; standalone results materialize as capped read-only spill ranges, resolve scalar spill-neighbor references, and report `#SPILL!` for bounded destination collisions.
- [ ] Conditional-formatting sidebar can author data-bar and color-scale rules and those rules survive XLSX round-trip.
- [ ] Structured table names, ranges, headers, styles, and totals-row flags persist through XLSX round-trip.
- [ ] What-if scenarios can capture selected cells, apply with recalculation, undo, and delete; native scenario/input-cell metadata survives XLSX round-trip.
- [ ] Slicers can be created from a selected table range, toggle values, clear to all values, persist through `.redoc`, and report their native-XLSX fallback.
- [ ] Chart sidebar binds to a range and renders bar/line/pie.
- [ ] Insert a bounded PNG/JPEG/GIF/WebP into the active Calc cell, verify it renders after reload; valid PNG/JPEG sources round-trip as native XLSX drawings while GIF/WebP or malformed sources produce a clear compatibility warning.
- [ ] CSV import and export round-trip quoted values and formulas-as-text.
- [ ] Keyboard navigation and selection remain usable at high zoom.
- [ ] Sheet print (selection / whole sheet) produces expected output.

## Presentation

- [ ] Slides can be added, selected, moved, and reordered.
- [ ] Text, shapes (rect/rounded rectangle/ellipse/triangle/diamond/star/arrow), images, tables, and charts survive save/reopen; common native PPTX shape presets and table parts reopen as editable elements.
- [ ] 8-handle resize and zoom behave predictably.
- [ ] Layout masters apply placeholder elements.
- [ ] Supported slide layouts (title, title/content, section, two-column, picture/caption, and blank) survive PPTX export/import and retain their editable layout choice.
- [ ] Slide transition selection (none/fade/slide left/slide right/wipe left/wipe right/zoom/dissolve/morph) applies in the editor and presenter and survives save/reopen and PPTX round-trip.
- [ ] Context menu Bring Forward / Send Backward step the selected element one layer at a time; Bring to Front / Send to Back still jump to the extremes.
- [ ] Empty native picture placeholders remain visible as editable “Click to add Picture” shapes when imported without media; malformed media relationships produce a bounded warning without dropping the placeholder.
- [ ] Align, distribute, group, and ungroup work on multi-selection.
- [ ] Presenter window opens with current/next slide and speaker notes.
- [ ] Entrance fade and zoom animations step through in presenter view.
- [ ] Presenter timer can pause/resume and restart; Reveal all, Home/End, PageUp/PageDown, and R keyboard controls behave safely at deck boundaries.
- [ ] Entrance fade/zoom delay, duration, and reveal order survive `.redoc` and PPTX round-trip (values are clamped to the supported editor range).
- [ ] Select a slide element, apply a safe HTTP(S)/mailto/tel hyperlink in Properties, and verify Ctrl/Cmd-click opens it; unsafe or malformed targets show an inline error and PPTX round-trip preserves valid external links.
- [ ] Handout print (1/2/4/6 per page, optional notes) layouts correctly.
- [ ] PDF/PPTX export opens or reports a clear error.
- [ ] Export a document containing a known fallback (for example, a native-pivot gap or unsupported slide animation) and verify the post-export Compatibility report lists the deterministic note.
- [ ] Writer can author a bounded bookmark from the Insert menu/toolbar/context menu, and DOCX bookmarks plus same-document hyperlinks reopen as editable anchors without a "Bookmarks were skipped" warning.
- [ ] Insert → Table of Contents generates entries from headings, each entry links to its heading bookmark, and the TOC exports to DOCX/PDF through the normal paragraph/link paths.
- [ ] Settings → Enable Spellcheck toggles native squiggles on the Writer surface and the Calc cell editor (relaunch after toggling to confirm persistence).
- [ ] Calc pivot panel with a Column column set produces a cross-tab with a Grand Total row and column, refreshes with slicer selections, and removes cleanly with Undo.
- [ ] Chart type dropdown offers Bar/Line/Area/Scatter/Pie/Doughnut; exported XLSX reopens with the matching native chart part.
- [ ] Writer hyperlink entry rejects `javascript:`/`data:` and control-character targets while accepting supported HTTPS/HTTP, mailto, tel, and internal bookmark links.
- [ ] Clicking an internal Writer hyperlink selects and scrolls to the matching bookmark, while a missing bookmark does not crash or trap the editor.

## Packaging and OS integration

- [ ] Double-click / “Open with” a `.redoc` document (doc, sheet, and slide variants) launches Redoc in the matching editor mode.
- [ ] Opening a second `.redoc` while Redoc is running loads that file into the correct mode (where the OS delivers the open event).
- [ ] Release installer artifacts are ≤ 55 MB (`pnpm quality:check` after `tauri build`, or release CI).
- [ ] `pnpm quality:check` passes after `pnpm build` (frontend gzip budget, generated bindings, capability-ledger validation, config gates).
- [ ] Opening `.pptx` imports supported text, shapes, media, notes, transitions, and native fade/zoom entrance effects; unsupported graphic frames remain visible with a compatibility warning.

## Recovery and failure handling

- [ ] Edit a document, wait for autosave, terminate the app, and verify recovery on next launch.
- [ ] Cancel native open/save dialogs without changing the current document.
- [ ] Corrupt or unsupported files show an error without crashing or overwriting user data.

## Automated tests (run via CI / `pnpm test` / `cargo test`)

These have unit or integration coverage in the repo today:

- [x] Formula parser and evaluator property tests (`crates/formula`, proptest).
- [x] Formula eval edge cases + dep_graph cycle detection (`crates/formula/src/eval.rs`, `dep_graph.rs`).
- [x] Generated schema catalog parity between `crates/doc-engine` and the editor's ProseMirror spec (`packages/doc-editor/src/schemaCatalog.test.ts`, `crates/doc-engine/src/schema.rs::exports_schema_catalog_json`).
- [x] Spellcheck setting resolves on/off on the Writer surface and Calc cell input (`packages/doc-editor/src/spellcheck.test.ts`, `packages/doc-editor/src/DocEditor.tsx`, `packages/sheet-editor/src/SheetEditor.tsx`).
- [x] Auto-TOC generation collects, sanitizes, deduplicates, and bounds heading anchors and renders internal-link entries (`packages/doc-editor/src/toc.test.ts`).
- [x] Multi-field pivots cross rows × columns with sum/count/average, grand totals, slicer-aware refresh, and group bounds (`packages/sheet-editor/src/pivotTables.test.ts`).
- [x] Chart designer persists all six chart types through workbook builds and maps them to native XLSX chart parts (`packages/sheet-editor/src/sheetModel.test.ts::persists_the_six_designer_chart_types_through_workbook_builds`, `crates/export/src/xlsx.rs::chart_type_for`).
- [x] Cell comments create/resolve/reopen/delete with bounds and per-cell listing (`packages/sheet-editor/src/cellComments.test.ts`).
- [x] Slide comments normalize malformed entries, round-trip through deck JSON, and attach to the active slide with resolve/reopen/delete (`packages/slide-editor/src/deckNormalize.test.ts`, `packages/slide-editor/src/SlideEditor.tsx`).
- [x] Slide comments export to native PPTX `ppt/comments` parts with author metadata (`p:cmAuthorLst`) and re-import with text, author, and resolved state intact (`crates/export/src/pptx.rs::round_trips_slide_comments_with_author_metadata`).
- [x] Container round-trip tests for doc, sheet, and slide bodies (`crates/file-io/tests/roundtrip.rs`).
- [x] Corrupt ZIP returns typed error, not panic (`corrupt_zip_returns_error_not_panic`).
- [x] Missing asset reference handled on load (`missing_asset` test).
- [x] Schema migration v0 → current (`crates/file-io/src/migrations.rs`).
- [x] Recovery `clear_all_snapshots` (`crates/core/src/recovery.rs`).
- [x] Size-bounded non-blocking log rotation preserves records and caps rotated backups (`crates/core/src/logging.rs`).
- [x] Base64 encoding/decoding and TSV parse/serialize (`packages/utils/src/index.test.ts`).
- [x] Sheet undo/history manager (`packages/sheet-editor/src/sheetHistory.test.ts`).
- [x] Sheet workbook model builder (`packages/sheet-editor/src/sheetModel.test.ts`).
- [x] Slicer value selection and row filtering (`packages/sheet-editor/src/slicers.test.ts`).
- [x] Doc HTML paste sanitizer (`packages/doc-editor/src/pasteSanitizer.test.ts`).
- [x] Bounded document compare summary (`packages/doc-editor/src/compare.test.ts`).
- [x] DOCX section-column round-trip (`round_trips_docx_section_columns`).
- [x] DOCX per-section page properties round-trip (`exports_per_section_page_properties`).
- [x] DOCX PAGE/NUMPAGES field-code round-trip (`round_trips_native_page_fields_in_document_body`).
- [x] DOCX paragraph spacing export (`exports_bounded_paragraph_spacing`).
- [x] DOCX paragraph layout round-trip (`round_trips_paragraph_layout_attributes`).
- [x] Unsupported DOCX simple fields warn and do not leak cached text (`skips_unsupported_simple_field_with_warning`).
- [x] Bounded Calc array functions (`test_array_shape_functions`, `test_bounded_dynamic_array_functions`), dynamic spill materialization, collision recovery, and local/cross-sheet neighbor references (`materializes_bounded_dynamic_array_spills`, `materializes_sequence_spills_with_expected_values`, `materializes_randarray_spills_with_bounded_shape`, `materializes_stacked_dynamic_arrays_with_padding`, `materializes_selected_reference_array_spills`, `materializes_sortby_spills_in_key_order`, `materializes_flattened_array_spills_with_column_scan`, `reports_spill_collision_and_rechecks_after_blocker_is_cleared`, `formulas_can_read_materialized_spill_neighbors`, `cross_sheet_formulas_follow_spill_neighbor_changes`), slicer-filtered pivot refresh (`pivotTables.test.ts`, `sheetModel.test.ts`), compatibility helpers (`test_text_and_boolean_compatibility_helpers`, `test_bounded_statistical_compatibility_helpers`), and generated 137-name formula catalog.
- [x] PPTX supported slide-layout round-trip (`round_trips_supported_slide_layouts`).
- [x] Empty native PPTX title/body placeholders become editable text placeholders (`imports_empty_native_placeholders_as_editable_text`).
- [x] Empty native PPTX picture placeholders remain editable (`preserves_empty_native_picture_placeholders_as_editable_shapes`).
- [x] Large-sheet warm recalculation budget (`scripts/perf-budget.mjs`, `grid_render` bench).
- [x] Shortcut registry / buildMenus (`packages/editor-common/src/ShortcutRegistry.test.ts`).
- [x] Slide deck normalize/toDeck round-trip (`packages/slide-editor/src/deckNormalize.test.ts`).
- [x] Slide geometry, align, group helpers (`packages/slide-editor/src/geometry.test.ts`).
- [x] Shell a11y axe scans for HomeScreen, Dialog, CommandPalette, FindBar (`tests/a11y/shell.a11y.test.tsx`).
- [x] Weekly nightly formula fuzz workflow (`.github/workflows/nightly-fuzz.yml`).
