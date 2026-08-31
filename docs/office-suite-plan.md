# Redoc Office Suite delivery plan

## Outcome

Deliver one efficient desktop application with a shared shell and three interoperable editors:

- Writer: document editing, layout, review, comments, tracked changes, search/replace, printing, PDF, and DOCX.
- Calc: formulas, formatting, charts, tables, filters, conditional formatting, pivot summaries, validation, printing, PDF, CSV, and XLSX.
- Slides: layouts, masters, shapes, media, charts, notes, transitions, entrance animations, presenter playback, PDF, and PPTX.

The application must remain useful when an Office feature is unsupported: preserve what can be preserved, return a clear compatibility warning, and never lose the original file or crash the process.

## Architecture

1. Shared desktop shell
   - One command bus, menu/shortcut registry, tabbed document sessions, dirty-state tracking, autosave, recovery, settings, and accessibility primitives.
   - Lazy-load editor panes and keep each document session isolated so switching files cannot leak state.

2. Canonical editor models
   - ProseMirror-compatible JSON for Writer.
   - Typed workbook/sheet/cell models for Calc.
   - Typed deck/slide/element models for Slides.
   - Version every model and provide migrations for older `.redoc` documents.

3. File and package boundary
   - `.redoc` is an atomic container with bounded JSON, archive entries, media, paths, and hashes.
   - Format adapters are isolated from editors: DOCX, XLSX, PPTX, PDF, CSV, and clipboard conversions.
   - Unsupported constructs are collected into a structured compatibility report rather than silently discarded.

4. Verification infrastructure
   - Pure unit tests for model transforms and formula/aggregation logic.
   - Fixture tests for Office package import/export and round trips.
   - Shell accessibility tests, performance budgets, security/fuzz tests, and release smoke tests.

## Phased implementation

### Phase 0 — contract and baseline

- Maintain `docs/capability-ledger.json` as the source of truth.
- Record every feature as absent, partial, implemented, verified, or interoperable.
- Establish `pnpm typecheck`, `pnpm test`, `pnpm build`, clippy, accessibility, performance, formula-catalog, and ledger gates.
- Define fixture corpus and a loss-report format before adding new adapters.

### Phase 1 — shell and reliability

- Complete multi-document sessions, mode-aware commands, tab lifecycle, close confirmations, and pane isolation.
- Finish autosave/recovery, clean-shutdown markers, atomic replacement, backup restore, and failed-save cleanup.
- Bound all archive/file/XML/media reads; reject unsafe paths and malformed containers with typed errors.
- Add structured logging and user-safe error messages around every I/O and backend command.

### Phase 2 — Writer

- Stabilize the document schema for paragraphs, headings, lists, tables, images, page breaks, layout attributes, and marks.
- Finish formatting, styles, page setup, headers/footers, print preview, find/replace, clipboard sanitization, and image handling.
- Complete review: anchored comments, insertion/deletion marks, mapping across edits, resolve/reopen, accept/reject, and review summaries.
- Add DOCX import/export for styles, hyperlinks, tables, media, comments, and native tracked-change mapping; emit explicit warnings for unsupported fields, sections, and revisions.
- Add document-to-PDF visual fixtures and round-trip tests.

### Phase 3 — Calc

- Expand the shared formula catalog and evaluator with reference, lookup, date, statistical, text, financial, array, and error semantics.
- Finish grid editing, fill/sort/filter, merges, freeze panes, number formats, validation, named ranges, and chart placement.
- Complete conditional formatting (predicates, data bars, color scales), pivot summaries, table metadata, slicer/scenario design, and refresh behavior.
- Preserve workbook metadata through every backend command and add XLSX import/export fixtures for formulas, styles, charts, tables, and compatibility reports.
- Keep recalculation within the warm-path budget and add large-sheet stress cases.

### Phase 4 — Slides

- Finish slide layouts, masters/placeholders, themes, shapes/connectors, tables, charts, images, notes, and selection geometry.
- Persist slide transitions and element entrance timing/order; complete presenter controls, keyboard navigation, timer, and reveal sequencing.
- Add PPTX import/export for media, notes, masters, transitions, and the supported animation subset with loss reporting for unsupported timelines/triggers.
- Add slide PDF fixtures and visual smoke tests for representative decks.

### Phase 5 — interoperability and migrations

- Define deterministic import normalization and export normalization for every format.
- Add versioned migrations for `.redoc` models and preserve unknown-but-safe fields where possible.
- Run corpus-based round trips: source → Redoc → source-compatible export → re-import, comparing semantic content and reporting expected loss.
- Keep native Office constructs that cannot be edited as visible, non-destructive compatibility annotations when practical.

### Phase 6 — performance, security, and resilience

- Profile startup, editor switching, large documents/sheets/decks, formula recalculation, canvas redraw, package parsing, and exports.
- Use lazy parsing, bounded caches, incremental redraw/recalc, debounced autosave, and cancellation for long operations.
- Fuzz parsers and formula inputs; enforce resource limits, safe path resolution, data-URI/media quotas, and panic boundaries.
- Add telemetry hooks that are opt-in, privacy-preserving, and useful for diagnosing failures.

### Phase 7 — release readiness

- Run the complete quality matrix on Windows packaging artifacts and clean machines.
- Verify accessibility (keyboard-only, focus order, labels, contrast), crash recovery, interrupted saves, read-only/newer-file behavior, and upgrade migrations.
- Publish a compatibility matrix for supported Office constructs and known limitations.
- Stage rollout with a rollback-capable installer and preserve user documents across upgrades.

## Definition of done for each capability

Every ledger entry must have:

1. A user-visible path in the correct editor.
2. A pure test for the core transform or evaluator.
3. A persistence or package fixture when applicable.
4. An explicit failure/unsupported-path test.
5. Passing typecheck, build, relevant unit tests, and the global quality gates.
6. Updated compatibility documentation and no unbounded resource path.

The suite is complete only when all three editors meet those criteria and the remaining Office differences are documented, non-destructive, and covered by loss reports.

