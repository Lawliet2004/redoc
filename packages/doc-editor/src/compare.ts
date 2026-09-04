import type { DocContent } from "./types";

export type CompareSegmentKind = "equal" | "insert" | "delete";

export interface CompareSegment {
  kind: CompareSegmentKind;
  text: string;
}

export interface DocumentCompareResult {
  beforeText: string;
  afterText: string;
  segments: CompareSegment[];
  insertedWords: number;
  deletedWords: number;
  changed: boolean;
  truncated: boolean;
}

export interface CompareSource {
  id: string;
  title: string;
  content: DocContent | null;
}

/** Resolve a selectable open-document baseline without ever returning the current document implicitly. */
export function findCompareSource(
  sources: readonly CompareSource[] | undefined,
  sourceId: string,
): CompareSource | null {
  if (!sourceId || !sources) return null;
  return sources.find((source) => source.id === sourceId && source.content != null) ?? null;
}

const MAX_TOKENS = 2_000;
const BLOCK_NODES = new Set([
  "blockquote",
  "code_block",
  "heading",
  "list_item",
  "paragraph",
  "table_cell",
  "table_row",
]);

function textFromDoc(doc: DocContent | null | undefined): string {
  const chunks: string[] = [];
  const visit = (node: DocContent) => {
    if (node.type === "text" && typeof node.text === "string") chunks.push(node.text);
    if (Array.isArray(node.content)) node.content.forEach(visit);
    if (BLOCK_NODES.has(node.type) && chunks.at(-1) !== "\n") chunks.push("\n");
  };
  if (doc) visit(doc);
  return chunks.join("").replace(/\n+$/u, "");
}

function tokens(text: string): string[] {
  return text.match(/\s+|[^\s]+/gu) ?? [];
}

function wordCount(text: string): number {
  return text.match(/[^\s]+/gu)?.length ?? 0;
}

function appendSegment(segments: CompareSegment[], kind: CompareSegmentKind, text: string) {
  if (!text) return;
  const last = segments.at(-1);
  if (last?.kind === kind) last.text += text;
  else segments.push({ kind, text });
}

function boundedDiff(before: string, after: string): { segments: CompareSegment[]; truncated: boolean } {
  const oldTokens = tokens(before);
  const newTokens = tokens(after);
  if (oldTokens.length > MAX_TOKENS || newTokens.length > MAX_TOKENS) {
    let prefix = 0;
    while (prefix < oldTokens.length && prefix < newTokens.length && oldTokens[prefix] === newTokens[prefix]) prefix += 1;
    let suffix = 0;
    while (
      suffix < oldTokens.length - prefix &&
      suffix < newTokens.length - prefix &&
      oldTokens[oldTokens.length - suffix - 1] === newTokens[newTokens.length - suffix - 1]
    ) suffix += 1;
    const segments: CompareSegment[] = [];
    appendSegment(segments, "equal", oldTokens.slice(0, prefix).join(""));
    appendSegment(segments, "delete", oldTokens.slice(prefix, oldTokens.length - suffix).join(""));
    appendSegment(segments, "insert", newTokens.slice(prefix, newTokens.length - suffix).join(""));
    appendSegment(segments, "equal", oldTokens.slice(oldTokens.length - suffix).join(""));
    return { segments, truncated: true };
  }

  const width = newTokens.length + 1;
  const table = new Uint16Array((oldTokens.length + 1) * width);
  for (let oldIndex = oldTokens.length - 1; oldIndex >= 0; oldIndex -= 1) {
    for (let newIndex = newTokens.length - 1; newIndex >= 0; newIndex -= 1) {
      const index = oldIndex * width + newIndex;
      table[index] = oldTokens[oldIndex] === newTokens[newIndex]
        ? (table[(oldIndex + 1) * width + newIndex + 1] + 1)
        : Math.max(table[(oldIndex + 1) * width + newIndex], table[oldIndex * width + newIndex + 1]);
    }
  }

  const segments: CompareSegment[] = [];
  let oldIndex = 0;
  let newIndex = 0;
  while (oldIndex < oldTokens.length && newIndex < newTokens.length) {
    if (oldTokens[oldIndex] === newTokens[newIndex]) {
      appendSegment(segments, "equal", oldTokens[oldIndex]);
      oldIndex += 1;
      newIndex += 1;
    } else if (table[(oldIndex + 1) * width + newIndex] >= table[oldIndex * width + newIndex + 1]) {
      appendSegment(segments, "delete", oldTokens[oldIndex]);
      oldIndex += 1;
    } else {
      appendSegment(segments, "insert", newTokens[newIndex]);
      newIndex += 1;
    }
  }
  while (oldIndex < oldTokens.length) appendSegment(segments, "delete", oldTokens[oldIndex++]);
  while (newIndex < newTokens.length) appendSegment(segments, "insert", newTokens[newIndex++]);
  return { segments, truncated: false };
}

export function compareDocContent(
  before: DocContent | null | undefined,
  after: DocContent | null | undefined,
): DocumentCompareResult {
  const beforeText = textFromDoc(before);
  const afterText = textFromDoc(after);
  const { segments, truncated } = boundedDiff(beforeText, afterText);
  const insertedText = segments.filter((segment) => segment.kind === "insert").map((segment) => segment.text).join("");
  const deletedText = segments.filter((segment) => segment.kind === "delete").map((segment) => segment.text).join("");
  return {
    beforeText,
    afterText,
    segments,
    insertedWords: wordCount(insertedText),
    deletedWords: wordCount(deletedText),
    changed: beforeText !== afterText,
    truncated,
  };
}
