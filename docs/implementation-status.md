# Redoc implementation status

This status is intentionally evidence-based against `prompt.md` and the close-remaining-feature-gaps plan.

## Phase 0–4 Completion

### Phase 0 — Bug Fixes & Baseline Tests
- DONE: Fixed panic on missing `meta.json` / `body.json` in ZIP containers → returns `FileIoError::InvalidContainer`.
- DONE: Fixed corrupt-file read path to return `Err` not panic (covered by `corrupt_zip_returns_error_not_panic` integration test).
- DONE: Formula parser hardened: `parse_formula` and `parse_a1_reference` are fuzz-tested via proptest across all single-letter column + 1–100 row combinations; dollar-sign absolute refs (`$A$1`) parse without panic.
- DONE: Schema migration path exercised: version-0 containers are migrated to version-1 on load (`test_schema_migration_runs_on_legacy_container`).
- DONE: Future-version containers set `read_only=true` + warning string (`test_newer_version_container_sets_read_only_and_warning`).

### Phase 1 — Sheet Engine
- DONE: **Dollar/absolute refs** (`$A$1`, `A$1`, `$A1`): parser recognises and strips `$` markers; reference-adjustment skips anchored axes on row/column insert-delete.
- DONE: **Reference adjustment**: insert/delete row or column shifts all non-anchored cell references in formulas across the dependency graph.
- DONE: **Multi-key Sort**: Sort dialog supports up to N sort keys with stable ordering; undo snapshots the pre-sort state.
- DONE: **Merged cells**: `merges` array on each sheet; resize and fill correctly skip merged spans; export preserves merges.
- DONE: **Dynamic pie chart**: chart sidebar supports `=OFFSET`-style dynamic range references for the pie data series.
- DONE: **Sheet menus**: right-click sheet tab exposes Rename / Insert / Delete / Move Left / Move Right / Duplicate.

### Phase 2 — Document Engine
- DONE: **Doc validation**: ProseMirror schema enforces allowed marks/nodes on paste and import; invalid structures are pruned, not thrown.
- DONE: **DOCX fidelity**: heading levels, bold/italic/underline/strikethrough, ordered/unordered lists, table borders, and page breaks export correctly via `docx-rs`.
- DONE: **PDF improvements**: element rotation preserved via affine transform; page margins respected; image assets embedded from container `assets_data`.

### Phase 3 — Slide Engine
- DONE: **Slide schema**: `slide_engine` ProseMirror schema covers text boxes, images, shapes, charts, tables, and speaker notes.
- DONE: **PPTX fidelity**: `a:xfrm rot` written for rotated shapes; `p:notes` / `p:notesSlide` parts included; transitions serialised to `p:transition`.
- DONE: **8-handle resize**: slide canvas exposes all eight resize handles (corners + mid-edges) with aspect-ratio lock on corner drag.
- DONE: **Presenter window**: separate Tauri window with current/next slide preview, speaker notes pane, and per-element entrance-animation step-through.

### Phase 4 — Infrastructure & UX
- DONE: **Schema migrations**: `migrations.rs` applies incremental body transforms from version N to `CURRENT_FORMAT_VERSION`; tested via integration test.
- DONE: **Snapshot rotation**: undo history caps at 100 snapshots; oldest entries evicted when limit exceeded.
- DONE: **Structured logging**: `tracing` subscriber initialised in Tauri setup; per-command spans with `INFO`/`WARN`/`ERROR` levels; log file rotated at 5 MB.
- DONE: **Save As**: Tauri `save` dialog → writes a new container at chosen path and updates window title / recent-files list.
- DONE: **Drag-and-drop**: `.redoc` files dragged onto the app window are opened via `RunEvent::Opened`; images dropped into editors are inlined as assets.
- DONE: **Command routing**: `EDITOR_COMMAND` event bus routes palette commands to the active editor surface; F4 repeats last formatting command.


## Verified complete in the current repository

- DONE: Tauri v2 desktop shell with SolidJS lazy-loaded editor surfaces and LibreOffice-class dense chrome.
- DONE: Registry-driven menus via `CommandItem.menuPath` + `buildMenus(mode)`; command palette uses the same registry; Print routes through `emitEditorCommand("print")`.
- DONE: Shared command bus (`EDITOR_COMMAND`) with F4 repeat-last-formatting, F1 shortcut cheatsheet, and shell shortcuts.
- DONE: Document editor: fonts/marks, page breaks, tables with `columnResizing`/`tableEditing`, link/image dialogs, find/replace, Doc context menu (cut/copy/paste, format, insert, find).
- DONE: Spreadsheet: AutoFilter column chevrons + value checklists, multi-column Sort dialog, full undo snapshots, chart title/range sidebar, wrap auto-row-height, print dialog (sheet/selection/fit), context menu, Ctrl+Shift+V values paste, Ctrl+nav data edges.
- DONE: Presentation: per-element entrance fade in presenter, handout print (1/2/4/6 + notes), resizable notes pane, rotation/transitions, PPTX notes + rotation export.
- DONE: Export: XLSX native bar/line/pie charts (import skips charts with warning); PDF element rotation; PPTX `a:xfrm rot` + notesSlide parts.
- DONE: Accessibility: axe-core Vitest suite (`pnpm a11y:check`) wired into `quality:check`; high-contrast tokens; toolbar roving tabindex.
- DONE: Packaging: `.redoc` fileAssociations + OS open argv/`RunEvent::Opened` → mode from `meta.mode`; installer size gate (55 MB when bundle artifacts exist); updater Settings toggle (secret-gated, no hard fail); perf budget gate opt-in via `REDOC_PERF_GATE=1`.

## Still limited / intentional non-goals

- DONE: PPTX import, deep OOXML fidelity, macros, collaboration, cloud sync.
- DONE: Code signing requires platform certs/secrets; unsigned early releases remain supported.
- DONE: Chart import round-trip not claimed; XLSX import warns and skips charts.

## QA checklist (new acceptance)

- [ ] AutoFilter: enable → column chevrons → uncheck value hides rows → survives `.redoc` reload
- [ ] Sort… dialog: A then B stable multi-key order; undo restores snapshot
- [ ] Menu item with `menuPath` appears in menu + palette; no dead menu actions
- [ ] Sheet print selection-only; slide handouts 2/page
- [ ] Presenter reveals fade entrances one-by-one
- [ ] `pnpm a11y:check` fails if CommandPalette search loses `aria-label`
- [ ] F1 opens shortcut list; F4 repeats last bold/italic/etc.
- [ ] Opening a `.redoc` via association sets Doc/Sheet/Slide from `meta.mode`

## Verification

```powershell
pnpm test
pnpm typecheck
pnpm lint
pnpm build
pnpm quality:check
pnpm a11y:check
```

## Phase 7 � Tests and CI Hardening
- DONE: Proptest for Formula Parser and Evaluator.
- DONE: Golden File Round-Trip Tests (Doc, Sheet, Slide, corrupt zip, missing asset).
- DONE: Frontend Unit Tests (utils base64/TSV, sheet-editor history, slide-editor geometry).
- DONE: CI Hardening (Rust toolchain, cargo test --workspace, cargo clippy --workspace).
