export type TrackedChangeKind = "insert" | "delete";
export type TrackedChangeDecision = "accept" | "reject";

export const TRACK_INSERT_MARK = "trackInsert" as const;
export const TRACK_DELETE_MARK = "trackDelete" as const;

export interface TrackedChangeStats {
  insertions: number;
  deletions: number;
  insertedCharacters: number;
  deletedCharacters: number;
}

export interface TrackedChangeSummary {
  id: string;
  kind: TrackedChangeKind;
  author: string;
  text: string;
  characters: number;
}

function markType(kind: TrackedChangeKind) {
  return kind === "insert" ? TRACK_INSERT_MARK : TRACK_DELETE_MARK;
}

function hasMark(node: any, type: string) {
  return Array.isArray(node?.marks) && node.marks.some((mark: any) => mark?.type === type);
}

function trackedMark(node: any): { kind: TrackedChangeKind; id: string; author: string } | null {
  if (!Array.isArray(node?.marks)) return null;
  const mark = node.marks.find((candidate: any) =>
    candidate?.type === TRACK_INSERT_MARK || candidate?.type === TRACK_DELETE_MARK,
  );
  if (!mark) return null;
  const kind = mark.type === TRACK_INSERT_MARK ? "insert" : "delete";
  const id = typeof mark.attrs?.changeId === "string" ? mark.attrs.changeId.trim() : "";
  if (!id) return null;
  return {
    kind,
    id,
    author: typeof mark.attrs?.author === "string" && mark.attrs.author.trim() ? mark.attrs.author : "Unknown",
  };
}

/** Count tracked text runs without depending on editor state or DOM. */
export function getTrackedChangeStats(doc: any): TrackedChangeStats {
  const stats: TrackedChangeStats = {
    insertions: 0,
    deletions: 0,
    insertedCharacters: 0,
    deletedCharacters: 0,
  };
  const visit = (node: any) => {
    if (!node || typeof node !== "object") return;
    if (node.type === "text" && typeof node.text === "string") {
      if (hasMark(node, TRACK_INSERT_MARK)) {
        stats.insertions += 1;
        stats.insertedCharacters += node.text.length;
      }
      if (hasMark(node, TRACK_DELETE_MARK)) {
        stats.deletions += 1;
        stats.deletedCharacters += node.text.length;
      }
    }
    if (Array.isArray(node.content)) node.content.forEach(visit);
  };
  visit(doc);
  return stats;
}

/** Collect individually actionable changes, coalescing adjacent runs with the same id. */
export function getTrackedChangeSummaries(doc: any): TrackedChangeSummary[] {
  const summaries = new Map<string, TrackedChangeSummary>();
  const visit = (node: any) => {
    if (!node || typeof node !== "object") return;
    if (node.type === "text" && typeof node.text === "string") {
      const mark = trackedMark(node);
      if (mark) {
        const key = `${mark.kind}:${mark.id}`;
        const existing = summaries.get(key);
        if (existing) {
          existing.text += node.text;
          existing.characters += node.text.length;
        } else {
          summaries.set(key, {
            id: mark.id,
            kind: mark.kind,
            author: mark.author,
            text: node.text,
            characters: node.text.length,
          });
        }
      }
    }
    if (Array.isArray(node.content)) node.content.forEach(visit);
  };
  visit(doc);
  return [...summaries.values()];
}

/**
 * Resolve tracked text in a JSON document. Deleted text is removed on accept,
 * inserted text is removed on reject; the surviving change marks are cleared.
 */
function resolveTrackedChangesInternal(doc: any, decision: TrackedChangeDecision, targetId?: string): any {
  const resolveNode = (node: any): any => {
    if (!node || typeof node !== "object") return node;
    const insert = hasMark(node, TRACK_INSERT_MARK);
    const deletion = hasMark(node, TRACK_DELETE_MARK);
    const targetMatches = targetId === undefined || node.marks?.some((mark: any) =>
      (mark?.type === TRACK_INSERT_MARK || mark?.type === TRACK_DELETE_MARK)
      && mark?.attrs?.changeId === targetId,
    );
    if (node.type === "text" && targetMatches && ((decision === "accept" && deletion) || (decision === "reject" && insert))) {
      return null;
    }

    const next = { ...node };
    if (Array.isArray(node.marks)) {
      const cleared = node.marks.filter((mark: any) => {
        const tracked = mark?.type === TRACK_INSERT_MARK || mark?.type === TRACK_DELETE_MARK;
        const remove = tracked && (targetId === undefined || mark?.attrs?.changeId === targetId);
        return !remove;
      });
      if (cleared.length) next.marks = cleared;
      else delete next.marks;
    }
    if (Array.isArray(node.content)) {
      next.content = node.content.map(resolveNode).filter(Boolean);
    }
    return next;
  };
  return resolveNode(doc);
}

export function resolveTrackedChanges(doc: any, decision: TrackedChangeDecision): any {
  return resolveTrackedChangesInternal(doc, decision);
}

/** Resolve only the tracked change carrying the supplied stable change id. */
export function resolveTrackedChange(doc: any, changeId: string, decision: TrackedChangeDecision): any {
  const id = changeId.trim();
  return id ? resolveTrackedChangesInternal(doc, decision, id) : doc;
}

/**
 * Fresh tracked-change mark attributes for the auto-tracking plugin. Kept
 * pure (no ProseMirror imports) so unit tests can exercise it directly.
 */
export function newTrackedChangeAttrs(kind: TrackedChangeKind, author = "You") {
  return {
    kind,
    author,
    changeId: `change-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`,
    createdAt: new Date().toISOString(),
  };
}

/**
 * Ranges of text inserted by a transaction, derived from its replace steps.
 * Each entry is {from, to} in the NEW document coordinates, ready for addMark.
 * Uses StepMap.forEach: the step's own map reports where inserted content
 * landed (source range -1 -> target range 0..len).
 */
export function insertedRangesFromTransaction(tr: any): Array<{ from: number; to: number }> {
  const ranges: Array<{ from: number; to: number }> = [];
  tr.steps.forEach((step: any) => {
    if (!step?.getMap) return;
    if (step.slice && (step.slice.size || 0) === 0) return;
    step.getMap().forEach((fromA: number, _fromB: number, toA: number, toB: number) => {
      // Inserted content occupies [toA, toB) in the new document when the
      // step replaced a collapsed or non-empty range with new content.
      if (toB > toA) {
        ranges.push({ from: toA, to: toB });
      }
    });
  });
  return ranges;
}

/**
 * Text deleted by a transaction, derived from its replace-step maps.
 * Each entry is {from, to, text} in OLD document coordinates (before the
 * transaction), ready for restore-and-mark inside appendTransaction.
 */
export function deletedRangesFromTransaction(
  tr: any,
  oldDoc: { nodesBetween: (from: number, to: number, fn: (node: any, pos: number) => void) => void },
): Array<{ from: number; to: number }> {
  const ranges: Array<{ from: number; to: number }> = [];
  tr.steps.forEach((step: any) => {
    if (!step?.getMap) return;
    const sliceSize = step.slice ? (step.slice.size || 0) : 1;
    if (sliceSize !== 0) return;
    step.getMap().forEach((fromA: number, fromB: number, _toA: number, _toB: number) => {
      // A step that replaced [fromA, fromB) with nothing deleted that span
      // of the old document.
      if (fromB > fromA) {
        ranges.push({ from: fromA, to: fromB });
      }
    });
  });
  return ranges.filter((range) => {
    // Only ranges that contained text in the old doc count; block-level
    // deletions (whole nodes) are left to the user to mark deliberately.
    let hasText = false;
    try {
      oldDoc.nodesBetween(range.from, range.to, (node: any) => {
        if (node.isText) hasText = true;
        return true;
      });
    } catch {
      return false;
    }
    return hasText;
  });
}