import type { DocContent } from "./types";

export const MAX_FOOTNOTES = 256;
export const MAX_FOOTNOTE_TEXT = 2_000;

export interface Footnote {
  id: string;
  /** 1-based display number, also used as the reference label. */
  label: number;
  text: string;
}

/** Read the doc-level footnotes array; bounded and order-normalized. */
export function readFootnotes(doc: DocContent | null | undefined): Footnote[] {
  const raw = (doc as Record<string, unknown> | null | undefined)?.footnotes;
  if (!Array.isArray(raw)) return [];
  return (raw as Footnote[])
    .filter((note) => note && typeof note.id === "string" && note.id.length > 0)
    .slice(0, MAX_FOOTNOTES)
    .map((note, index) => ({
      id: note.id,
      label: Number.isFinite(note.label) ? Math.max(1, Math.floor(Number(note.label))) : index + 1,
      text: typeof note.text === "string" ? note.text.slice(0, MAX_FOOTNOTE_TEXT) : "",
    }));
}

/** Renumber footnotes sequentially and drop notes with no text and no refs. */
export function normalizeFootnotes(notes: Footnote[], referencedIds: Set<string>): Footnote[] {
  const kept = notes.filter((note) => note.text.trim().length > 0 || referencedIds.has(note.id));
  const seen = new Set<string>();
  let label = 0;
  return kept
    .filter((note) => (seen.has(note.id) ? false : (seen.add(note.id), true)))
    .slice(0, MAX_FOOTNOTES)
    .map((note) => ({ ...note, label: (label += 1) }));
}

export function newFootnoteId(): string {
  try {
    return `fn-${crypto.randomUUID()}`;
  } catch {
    return `fn-${Date.now()}-${Math.floor(Math.random() * 1e6)}`;
  }
}

/** Collect footnote ids referenced anywhere in the serialized doc body. */
export function collectFootnoteRefIds(node: DocContent | null | undefined, ids: Set<string> = new Set()): Set<string> {
  if (!node || typeof node !== "object") return ids;
  if (node.type === "footnote_ref" && typeof node.attrs?.id === "string") {
    ids.add(node.attrs.id as string);
  }
  if (Array.isArray(node.content)) {
    for (const child of node.content) collectFootnoteRefIds(child, ids);
  }
  return ids;
}

/** Sync the doc-level footnotes array: labels match body reference order. */
export function syncFootnotesWithRefs(
  notes: Footnote[],
  doc: DocContent | null | undefined,
): Footnote[] {
  const refIds = collectFootnoteRefIds(doc);
  const byId = new Map(notes.map((note) => [note.id, note]));
  let label = 0;
  const ordered: Footnote[] = [];
  const used = new Set<string>();
  const walk = (node: DocContent | null | undefined) => {
    if (!node || typeof node !== "object") return;
    if (node.type === "footnote_ref" && typeof node.attrs?.id === "string") {
      const id = node.attrs.id as string;
      if (!used.has(id)) {
        used.add(id);
        label += 1;
        const existing = byId.get(id);
        ordered.push({ id, label, text: existing?.text ?? "" });
      }
    }
    if (Array.isArray(node.content)) {
      for (const child of node.content) walk(child);
    }
  };
  walk(doc);
  // Keep orphan notes (text without a ref) so editing a ref away does not
  // silently destroy the user's note text; they re-attach if the ref returns.
  for (const note of notes) {
    if (!used.has(note.id) && note.text.trim() && ordered.length < MAX_FOOTNOTES) {
      label += 1;
      ordered.push({ ...note, label });
    }
  }
  return ordered.slice(0, MAX_FOOTNOTES);
}
