# Implementation status addendum

The historical audit snapshot in `implementation-status.md` predates the current interoperability work. The following capabilities are now implemented beyond that snapshot:

- PPTX import is available from the desktop open dialog, OS open events, and drag-and-drop. Text, basic shapes/connectors, embedded media, notes, canvas size, and basic transitions are preserved; unsupported graphic frames remain explicit warnings.
- XLSX chart parts are mapped through worksheet and drawing relationships into `ChartModel` entries with chart type, title, and placement anchors. Unsupported or malformed chart mappings remain non-fatal warnings.
- Exported bytes use an atomic write path with fsync, temporary-file cleanup, and backup/restore replacement fallback.
- DOCX package imports enforce file, archive-entry, XML-part, per-media, and total-media bounds; oversized media is skipped with an explicit compatibility warning.
- The desktop shell now keeps multiple independent document sessions in tabbed views. Each session retains its editor mode, content, path, backend id, and dirty/error state; closing a dirty tab requires confirmation and editor panes remount per session to prevent state leakage.
- Writer now supports persistent selection-anchored comments with a review sidebar, resolve/reopen/delete actions, and anchor mapping across edits. Tracked-change diffing and native DOCX comment round-trip remain future work.
- Calc conditional-formatting rules now survive workbook command round-trips and render on visible and merged cells for numeric/text predicates; pivot tables, slicers, scenarios, data bars, and color-scale authoring remain partial.

These features remain intentionally partial where Office fidelity requires advanced masters, placeholders, pivot charts, or animation timelines.
