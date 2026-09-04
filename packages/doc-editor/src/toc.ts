import type { DocContent } from "./types";

/**
 * Auto-TOC generation.
 *
 * Builds a table-of-contents as real document content — heading-styled
 * entries with internal bookmark hyperlinks to anchors on each heading — so
 * the TOC exports to DOCX/PDF/print via the existing paragraph/link paths
 * instead of needing a new node type or exporter branch.
 */

export interface TocEntry {
  level: number;
  text: string;
  anchor: string;
}

const TOC_MAX_HEADINGS = 200;
const TOC_MAX_TEXT = 120;
const TOC_LEVELS: [number, number] = [1, 6];

function normalizeAnchorName(text: string, used: Set<string>): string {
  const base = text
    .slice(0, 40)
    .replace(/[^A-Za-z0-9_]+/g, "_")
    .replace(/^_+|_+$/g, "")
    .replace(/^(\d)/, "H$1");
  let name = base || "Heading";
  let suffix = 2;
  while (used.has(name)) {
    name = `${base}_${suffix}`;
    suffix += 1;
  }
  used.add(name);
  return name;
}

/** Public anchor sanitizer used by the editor when attaching TOC bookmarks. */
export function sanitizeTocAnchor(text: string, used: Set<string>): string {
  return normalizeAnchorName(text, used);
}

/** Collect the headings of a document JSON body into bounded TOC entries. */
export function buildTocEntries(doc: DocContent | null | undefined): TocEntry[] {
  const entries: TocEntry[] = [];
  if (!doc || typeof doc !== "object") return entries;
  const used = new Set<string>();
  const walk = (node: DocContent) => {
    if (entries.length >= TOC_MAX_HEADINGS) return;
    if (node.type === "heading") {
      const level = Math.max(TOC_LEVELS[0], Math.min(TOC_LEVELS[1], Number(node.attrs?.level) || 1));
      const text = String(node.textContent ?? collectText(node)).trim().slice(0, TOC_MAX_TEXT);
      if (text) {
        entries.push({ level, text, anchor: normalizeAnchorName(text, used) });
      }
    }
    for (const child of node.content ?? []) walk(child);
  };
  for (const child of doc.content ?? []) walk(child);
  return entries;
}

function collectText(node: DocContent): string {
  let text = "";
  if (node.text) text += node.text;
  for (const child of node.content ?? []) text += collectText(child);
  return text;
}

/**
 * Find the position of the Nth heading node in a ProseMirror document
 * (0-based heading index), used to attach bookmark marks at heading text.
 */
export function headingTextPositions(
  nodes: Array<{ type: { name: string }; attrs: { level?: number } }>,
): number[] {
  const positions: number[] = [];
  nodes.forEach((node, index) => {
    if (node.type.name === "heading" && index >= 0) positions.push(index);
  });
  return positions;
}

/** Build TOC entry paragraphs as doc JSON (heading level + internal link mark). */
export function buildTocContent(entries: TocEntry[]): DocContent[] {
  return entries.map((entry) => ({
    type: "paragraph",
    attrs: { indent: Math.max(0, entry.level - 1) * 0.75, lineHeight: 1.35, spacingBefore: 0, spacingAfter: 0 },
    content: [
      {
        type: "text",
        text: entry.text,
        marks: [
          { type: "link", attrs: { href: `internal:${entry.anchor}` } },
        ],
      },
    ],
  }));
}
