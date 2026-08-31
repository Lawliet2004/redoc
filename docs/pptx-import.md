# PPTX import coverage

Redoc can open `.pptx` files through the desktop open dialog, operating-system open events, and drag-and-drop. The importer currently preserves:

- slide canvas size and slide ordering;
- text boxes with position, rotation, font, color, alignment, and basic marks;
- basic shapes and connectors;
- embedded media as data URIs;
- speaker notes; and
- fade and directional push transitions.

Unsupported graphic frames (including tables and charts) are skipped with warnings. Imported presentations are intentionally marked dirty and should be saved as `.redoc` before editing; native PPTX write-back remains a later interoperability milestone.
