# PPTX import coverage

Redoc can open `.pptx` files through the desktop open dialog, operating-system open events, and drag-and-drop. The importer currently preserves:

- slide canvas size and slide ordering;
- text boxes with position, rotation, font, color, alignment, and basic marks;
- basic shapes and connectors;
- embedded media as data URIs;
- speaker notes;
- native table parts and supported bar/line/pie chart parts;
- theme colors/fonts and slide-specific solid backgrounds; and
- fade, directional push, wipe, zoom, and dissolve transitions, plus bounded native entrance-fade and entrance-zoom timing.

Unsupported shape presets, graphic frames, chart types, and advanced animation timelines are preserved as bounded placeholders or safe fallbacks with compatibility warnings. Imported presentations can be edited and exported back to PPTX for the supported subset; unsupported native constructs are not silently written back.

The importer bounds each XML part (16 MiB), each media part (32 MiB), total media (256 MiB), and archive entries (10,000) so malformed or hostile packages fail or degrade with a bounded warning instead of exhausting memory.
