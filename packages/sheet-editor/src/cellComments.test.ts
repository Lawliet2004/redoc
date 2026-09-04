import { describe, expect, it } from "vitest";
import {
  commentsForCell,
  makeCellComment,
  openCommentCount,
  removeCellComment,
  sanitizeCellCommentText,
  setCellCommentResolved,
  upsertCellComment,
} from "./cellComments";
import type { CellComment } from "./sheetTypes";

describe("cell comments", () => {
  it("creates a bounded, sanitized comment", () => {
    const result = makeCellComment(3, 2, "  Check this total  ", "Alice");
    if ("error" in result) throw new Error(result.error);
    expect(result.comment.row).toBe(3);
    expect(result.comment.col).toBe(2);
    expect(result.comment.text).toBe("Check this total");
    expect(result.comment.author).toBe("Alice");
    expect(result.comment.resolved).toBe(false);
  });

  it("rejects empty text and non-strings", () => {
    expect(makeCellComment(1, 1, "   ")).toHaveProperty("error");
    expect(sanitizeCellCommentText(undefined)).toBeNull();
    expect(sanitizeCellCommentText(42)).toBeNull();
  });

  it("upserts by id without duplicating", () => {
    const first = makeCellComment(1, 1, "One");
    if ("error" in first) throw new Error(first.error);
    const list = upsertCellComment([], first.comment);
    const edited: CellComment = { ...first.comment, text: "One (edited)" };
    const next = upsertCellComment(list, edited);
    expect(next).toHaveLength(1);
    expect(next[0].text).toBe("One (edited)");
  });

  it("resolves and reopens by id", () => {
    const made = makeCellComment(2, 2, "Fix");
    if ("error" in made) throw new Error(made.error);
    const resolvedList = setCellCommentResolved([made.comment], made.comment.id, true);
    expect(resolvedList[0].resolved).toBe(true);
    expect(openCommentCount(resolvedList)).toBe(0);
    const reopened = setCellCommentResolved(resolvedList, made.comment.id, false);
    expect(reopened[0].resolved).toBe(false);
    expect(openCommentCount(reopened)).toBe(1);
  });

  it("removes by id", () => {
    const made = makeCellComment(4, 4, "Gone");
    if ("error" in made) throw new Error(made.error);
    expect(removeCellComment([made.comment], made.comment.id)).toHaveLength(0);
  });

  it("lists per-cell comments unresolved first", () => {
    const a = makeCellComment(1, 1, "A");
    const b = makeCellComment(1, 1, "B");
    const c = makeCellComment(9, 9, "Other cell");
    if ("error" in a || "error" in b || "error" in c) throw new Error("make failed");
    let list = [a.comment, b.comment, c.comment];
    list = setCellCommentResolved(list, a.comment.id, true);
    const perCell = commentsForCell(list, 1, 1);
    expect(perCell).toHaveLength(2);
    expect(perCell[0].resolved).toBe(false);
    expect(commentsForCell(list, 5, 5)).toHaveLength(0);
  });

  it("enforces the global comment bound", () => {
    const many = Array.from({ length: 500 }, (_, i) => {
      const made = makeCellComment(i, 1, `c${i}`);
      if ("error" in made) throw new Error(made.error);
      return made.comment;
    });
    const overflow = makeCellComment(1, 1, "one too many", "You", many);
    expect(overflow).toHaveProperty("error");
  });
});
