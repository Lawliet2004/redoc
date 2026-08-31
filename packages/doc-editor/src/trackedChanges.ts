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

function markType(kind: TrackedChangeKind) {
  return kind === "insert" ? TRACK_INSERT_MARK : TRACK_DELETE_MARK;
}

function hasMark(node: any, type: string) {
  return Array.isArray(node?.marks) && node.marks.some((mark: any) => mark?.type === type);
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

/**
 * Resolve tracked text in a JSON document. Deleted text is removed on accept,
 * inserted text is removed on reject; the surviving change marks are cleared.
 */
export function resolveTrackedChanges(doc: any, decision: TrackedChangeDecision): any {
  const resolveNode = (node: any): any => {
    if (!node || typeof node !== "object") return node;
    const insert = hasMark(node, TRACK_INSERT_MARK);
    const deletion = hasMark(node, TRACK_DELETE_MARK);
    if (node.type === "text" && ((decision === "accept" && deletion) || (decision === "reject" && insert))) {
      return null;
    }

    const next = { ...node };
    if (Array.isArray(node.marks)) {
      const cleared = node.marks.filter((mark: any) => mark?.type !== TRACK_INSERT_MARK && mark?.type !== TRACK_DELETE_MARK);
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

