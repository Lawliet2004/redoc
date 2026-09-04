import type { CellComment } from "./sheetTypes";

/**
 * Cell comment operations (offline collaboration affordance).
 * Pure helpers so add/resolve/reopen/delete behavior is testable without the
 * canvas editor.
 */

const MAX_COMMENTS = 500;
const MAX_TEXT_CHARS = 2_000;

export function sanitizeCellCommentText(value: unknown): string | null {
  if (typeof value !== "string") return null;
  const trimmed = value.trim().slice(0, MAX_TEXT_CHARS);
  return trimmed ? trimmed : null;
}

export function makeCellComment(
  row: number,
  col: number,
  text: string,
  author = "You",
  existing: readonly CellComment[] = [],
): { comment: CellComment } | { error: string } {
  const safeText = sanitizeCellCommentText(text);
  if (!safeText) return { error: "Comment text is required." };
  if (existing.length >= MAX_COMMENTS) {
    return { error: `Comment limit of ${MAX_COMMENTS} reached.` };
  }
  const rowSafe = Math.max(0, Math.trunc(row));
  const colSafe = Math.max(0, Math.trunc(col));
  return {
    comment: {
      id: `comment-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`,
      row: rowSafe,
      col: colSafe,
      author: author.trim().slice(0, 80) || "You",
      text: safeText,
      resolved: false,
      createdAt: new Date().toISOString(),
    },
  };
}

export function upsertCellComment(
  comments: readonly CellComment[],
  comment: CellComment,
): CellComment[] {
  const next = comments.filter((existing) => existing.id !== comment.id);
  next.push(comment);
  return next;
}

export function setCellCommentResolved(
  comments: readonly CellComment[],
  id: string,
  resolved: boolean,
): CellComment[] {
  return comments.map((comment) =>
    comment.id === id ? { ...comment, resolved } : comment,
  );
}

export function removeCellComment(
  comments: readonly CellComment[],
  id: string,
): CellComment[] {
  return comments.filter((comment) => comment.id !== id);
}

/** Comments anchored to one cell, unresolved first (review ordering). */
export function commentsForCell(
  comments: readonly CellComment[],
  row: number,
  col: number,
): CellComment[] {
  return comments
    .filter((comment) => comment.row === row && comment.col === col)
    .sort((a, b) => Number(a.resolved) - Number(b.resolved));
}

/** Open (unresolved) comment count for the status bar / badges. */
export function openCommentCount(comments: readonly CellComment[]): number {
  return comments.filter((comment) => !comment.resolved).length;
}
