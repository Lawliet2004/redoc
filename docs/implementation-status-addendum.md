# Implementation status addendum

The historical audit snapshot in `implementation-status.md` predates the current interoperability work. The following capabilities are now implemented beyond that snapshot:

- PPTX import is available from the desktop open dialog, OS open events, and drag-and-drop. Text, basic shapes/connectors, embedded media, notes, canvas size, and basic transitions are preserved; unsupported graphic frames remain explicit warnings.
- XLSX chart parts are mapped through worksheet and drawing relationships into `ChartModel` entries with chart type, title, and placement anchors. Unsupported or malformed chart mappings remain non-fatal warnings.
- Exported bytes use an atomic write path with fsync, temporary-file cleanup, and backup/restore replacement fallback.

These features remain intentionally partial where Office fidelity requires advanced masters, placeholders, pivot charts, or animation timelines.
