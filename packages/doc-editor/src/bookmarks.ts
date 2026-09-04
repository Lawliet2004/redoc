import type { Node } from "prosemirror-model";

/** Word-compatible bookmark names are bounded and begin with a letter or underscore. */
export const MAX_BOOKMARK_NAME_CHARS = 40;

export function normalizeBookmarkName(value: string): string | null {
  const trimmed = value.trim();
  if (!trimmed) return null;

  const characters = Array.from(trimmed).slice(0, MAX_BOOKMARK_NAME_CHARS);
  const normalized = characters
    .map((character, index) => {
      if (index === 0 && !/^[A-Za-z_]$/.test(character)) return "_";
      return /^[A-Za-z0-9_]$/.test(character) ? character : "_";
    })
    .join("");
  return normalized || null;
}

export function findBookmarkPosition(doc: Node, requestedName: string): number | null {
  const name = normalizeBookmarkName(requestedName)?.toLowerCase();
  if (!name) return null;
  let found: number | null = null;
  doc.descendants((node, position) => {
    if (found !== null || !node.isText) return;
    if (node.marks.some((mark) => mark.type.name === "bookmark" && String(mark.attrs.name || "").toLowerCase() === name)) {
      found = position;
    }
  });
  return found;
}
