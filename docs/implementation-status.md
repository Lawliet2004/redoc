# Redoc implementation status (post-remediation audit)

Evidence-based snapshot of the current repository. Claims below are tied to source files, not roadmap intent.

## Verified working

### Shell (`apps/desktop`, `packages/editor-common`, `packages/ui`)

- **Home screen** with blank/open flows and template starters (`HomeScreen.tsx`).
- **Recents + pins** persisted via Tauri commands `get_recents` / `toggle_pin_recent` (`lib.rs`, `HomeScreen.tsx`).
- **Settings** dialog with autosave interval, theme (`SettingsDialog.tsx`, `App.tsx`).
- **Command palette** (`Ctrl+K`) and registry-driven menus from `registerCommands.ts` / `buildMenus`.
- **F1** opens keyboard-shortcuts dialog; **F4** repeats last formatting command (`registerCommands.ts`, `EditorCommands.ts`).
- **Drag-and-drop**: `.redoc` files via `tauri://drag-drop` and `RunEvent::Opened`; images dropped on home (`App.tsx`, `lib.rs`).
- **Save As** via native dialog, updates path/title/recents (`handleSaveAs` in `App.tsx`).
- **Per-mode buffers** when switching Doc/Sheet/Slide (`ModeBuffer`, `handleSwitchMode` in `App.tsx`).
- **Dirty close guard** before mode switch / new / open (`confirmDiscardIfDirty` in `App.tsx`).
- **Recovery prompt** with discard-all (`checkRecovery`, `discardRecoverySnapshots` in `App.tsx` / `lib.rs`).
- **Window title** reflects document name and dirty prefix (`setTitle` in `App.tsx`).
- **Scoped filesystem**: Tauri capabilities grant document read/write and app-data only (`capabilities/default.json`).
- **Accessibility**: axe-core Vitest suite (`tests/a11y/shell.a11y.test.tsx`, `pnpm a11y:check`).

### Sheet (`packages/sheet-editor`, `crates/sheet-engine`, `crates/formula`)

- **Canvas grid** with virtualization, hiDPI scaling (`devicePixelRatio`), damage-rect invalidation (`markGridDirtyRect`), and text-measure cache (`textMeasureCache.ts`, `SheetEditor.tsx`).
- **50+ formula functions** in `crates/formula` including **XLOOKUP**, **OFFSET**, **INDIRECT**, lookup/stat/date/text families (`functions.rs`, `eval.rs`).
- **AutoFilter** with column chevrons and value checklists (`SheetEditor.tsx`, `sheet-engine/workbook/filter.rs`).
- **Multi-key sort** dialog with stable ordering (`SortDialog.tsx`).
- **Merged cells** render/export path (`merges` signal, `renderMergedCell`).
- **Charts** bar/line/pie sidebar bound to sheet ranges (`chartType`, chart panel in `SheetEditor.tsx`).
- **Freeze panes** from selection (`freezeRows` / `freezeCols`).
- **Insert/delete rows and columns** via Rust commands (`insertRowsAt`, `insertColsAt`, etc.).
- **Find/replace** via shared `FindBar` (`findBarOpen`, `findReplaceMode`).
- **Named ranges** UI (`namedRanges` signal, add/delete handlers).
- **Number formats** dialog (`NumberFormatDialog.tsx`).
- **Data validation** lists (`DataValidationDialog.tsx`, formula-bar dropdown).
- **Hyperlinks** on cells (`cellHyperlink`, style.hyperlink).

### Document (`packages/doc-editor`, `crates/doc-engine`, `crates/export`)

- **ProseMirror editor** with custom schema built from `prosemirror-schema-basic` + lists/tables/images (`DocEditor.tsx`).
- **Marks**: bold, italic, underline, strike, link, highlight, etc.
- **Tables**: merge/split cells, column resizing, table editing plugin.
- **Find/replace** via `FindBar`.
- **Page setup** persisted in document JSON (`PageSetupDialog`, `pageSetup` in `buildDocJson`).
- **Print preview** (`PrintPreview.tsx`).
- **Headings H1�H6** in toolbar and schema.
- **Image resize** (drag handle) and **drop-to-insert** (`resizeSelectedImage`, `onDrop`).
- **Paste sanitization** (`pasteSanitizer.ts`, `transformPastedHTML`).
- **DOCX import**: lists, hyperlinks, highlight/color runs (`export/src/docx.rs` import path).
- **DOCX export**: numbering, `w:vertAlign`, table borders (`add_list_numbering`, border/numbering writers in `docx.rs`).

### Slides (`packages/slide-editor`, `crates/slide-engine`, `crates/export`)

- **Slide editor** restored with canvas, sidebar, and element model (`SlideEditor.tsx`).
- **8-handle resize** (nw/n/ne/e/se/s/sw/w) with shift-aspect lock (`ResizeHandle`, `SlideEditor.css`).
- **Zoom** control (10�200% scale transform).
- **Themes/layouts** sidebar with layout masters (`applyLayout`, theme color tokens).
- **Presenter window** as separate Tauri webview (`PresenterView.tsx`, `open_presenter_window` in `lib.rs`).
- **Table and chart elements** in slide model and renderer.
- **Align / distribute** selected elements (`distributeSelected`).
- **Group / ungroup** (`groupSelected`, `ungroupSelected`).
- **Shape library**: rect, rounded rect, ellipse, triangle, diamond, star, line, arrow (`shapeUtils.tsx`).
- **Handout print** 1/2/4/6 per page with optional speaker notes (`printPerPage`, print dialog).
- **PDF and PPTX export** from Rust (`export/pdf.rs`, `export/pptx.rs`).

### Infrastructure

- **Typed Tauri bindings** via `tauri-specta`; `pnpm generate:bindings` produces `packages/api-client/src/generated.ts`; quality gate enforces no direct `@tauri-apps/api` outside generated client.
- **Panic boundaries** on Tauri commands via `handle_panic!` macro wrapping `catch_unwind` (`lib.rs`).
- **Size-bounded log rotation** via non-blocking `SizeRollingWriter` (5 MiB active-file bound, 4 bounded rotated backups `redoc.log.1..4`; oversized single records stay intact, rotation failure reopens current file) (`crates/core/src/logging.rs`).
- **Single-instance app**: a second launch forwards its `.redoc` argv to the running instance over the existing `redoc-open-file` path and exits (`tauri-plugin-single-instance` in `apps/desktop/src-tauri/src/lib.rs`).
- **Atomic JSON state writes**: `settings.json`/`recents.json` go through temp+fsync+rename (backup-restore on failed replace) like document saves (`crates/core/src/state.rs`).
- **Magic-byte verification before import**: `.redoc`/`.docx`/`.xlsx`/`.pptx` must start with the ZIP local-file-header magic and `.csv` must be text, or the import returns a typed error (`lib.rs` `ensure_zip_magic`, `packages/editor-common/src/fileRouting.ts` `verifyMagicBytes`).
- **Read-only guard for newer formats**: opening a container with a newer `formatVersion` surfaces a non-blocking banner; Save is blocked with an explanatory dialog (Save As allowed) and the backend save refuses to silently downgrade (`container.rs`, `lib.rs`, `App.tsx`).
- **Quality gates**: gzipped frontend budget, generated bindings check, installer size gate, optional perf gate (`scripts/quality-gates.mjs`).

## Known limitations / non-goals

- **PPTX import is available** for text, shapes, media, notes, transitions, tables, and basic charts; unsupported graphic frames stay visible with compatibility warnings (see `implementation-status-addendum.md`).
- **No macros, pivot charts, real-time collaboration, or cloud sync** (out of scope for v1).
- **`zip` crate is NOT unified**: our crates (`redoc-core`, `redoc-file-io`, `redoc-export`) pin `zip` 2.4, but transitive dependencies pull additional major lines — `calamine`/`rust_xlsxwriter` resolve zip 2.4.2, `tauri-plugin-updater` 4.6.1, and `docx-rs` 8.6.0 (see `Cargo.lock`). Unifying requires upstream major-version migrations and is out of scope for v1.
- **DocEditor ProseMirror schema is parity-checked** against `crates/doc-engine` via the generated `schema-catalog.json` (`cargo test -p redoc-doc-engine` regenerates it; `schemaCatalog.test.ts` fails on drift).
- **Log rotation is size-bounded** (5 MiB active file + 4 rotated backups), not calendar-daily.

## Addendum (production-readiness pass)

- **Spell check**: the Settings toggle now drives the platform (WebView2) spellchecker on the Writer editing surface and the Calc cell input via the `spellcheck` DOM attribute — no bundled dictionary, native squiggles and suggestions.
- **Auto-TOC**: Insert → Table of Contents (toolbar, context menu, palette) generates bounded, deduplicated heading entries with bookmark anchors and `internal:` links, so DOCX/PDF/print export the TOC through existing paths.
- **Multi-field pivots**: the Calc pivot panel accepts an optional Column field producing a rows × columns cross-tab with grand totals, slicer-aware refresh, and a 500-group bound.
- **Chart designer**: six chart types (bar, line, area, scatter, pie, doughnut) render in the sheet SVG panel with title/axis labels and round-trip through native XLSX chart parts (`chart_type_for` covers all six; import sniffs doughnut/scatter/area markers).
- **Review comments in Calc and Slides**: cell-anchored comments in the spreadsheet (red corner-triangle markers, sidebar add/resolve/reopen/delete) and slide-anchored comments in the presentation notes pane — both persisted in `.redoc` and the Rust models (`CellCommentModel`, `SlideCommentModel`), and slide comments round-trip through native PPTX `p:cmLst` parts with `p:cmAuthorLst` attribution (export and import).
- **Theme v2**: deduplicated design tokens (spacing/radius/motion/shadows) with correct dark-default and light-theme overrides; the Home screen is fully class-based with modern micro-interactions (hover lift, focus rings, tokenized filters and file rows).

## QA checklist pointers

Manual acceptance steps live in [docs/qa-checklist.md](./qa-checklist.md). Run that checklist before release; unchecked items are intentional manual verification, not missing implementation claims.

## Verification commands

```powershell
pnpm typecheck
pnpm test
pnpm lint
pnpm build
pnpm quality:check
pnpm a11y:check
```

Rust workspace (also exercised in CI):

```powershell
cargo test --workspace
cargo clippy --workspace
```
