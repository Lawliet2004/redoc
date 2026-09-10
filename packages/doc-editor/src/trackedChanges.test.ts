import { describe, expect, it } from "vitest";
import { getTrackedChangeStats, getTrackedChangeSummaries, resolveTrackedChange, resolveTrackedChanges, newTrackedChangeAttrs, insertedRangesFromTransaction, deletedRangesFromTransaction } from "./trackedChanges";

const doc = {
  type: "doc",
  content: [{
    type: "paragraph",
    content: [
      { type: "text", text: "kept " },
      { type: "text", text: "new", marks: [{ type: "trackInsert", attrs: { author: "You", changeId: "insert-1" } }] },
      { type: "text", text: " old", marks: [{ type: "trackDelete", attrs: { author: "You", changeId: "delete-1" } }] },
    ],
  }],
};

describe("tracked changes", () => {
  it("counts tracked runs and characters", () => {
    expect(getTrackedChangeStats(doc)).toEqual({
      insertions: 1,
      deletions: 1,
      insertedCharacters: 3,
      deletedCharacters: 4,
    });
  });

  it("accepts deletions and clears surviving insertion marks", () => {
    expect(resolveTrackedChanges(doc, "accept")).toEqual({
      type: "doc",
      content: [{ type: "paragraph", content: [{ type: "text", text: "kept " }, { type: "text", text: "new" }] }],
    });
  });

  it("summarizes and resolves one change without touching the others", () => {
    expect(getTrackedChangeSummaries(doc)).toEqual([
      { id: "insert-1", kind: "insert", author: "You", text: "new", characters: 3 },
      { id: "delete-1", kind: "delete", author: "You", text: " old", characters: 4 },
    ]);
    expect(resolveTrackedChange(doc, "insert-1", "reject")).toEqual({
      type: "doc",
      content: [{ type: "paragraph", content: [{ type: "text", text: "kept " }, { type: "text", text: " old", marks: [{ type: "trackDelete", attrs: { author: "You", changeId: "delete-1" } }] }] }],
    });
  });

  it("rejects insertions and clears surviving deletion marks", () => {
    expect(resolveTrackedChanges(doc, "reject")).toEqual({
      type: "doc",
      content: [{ type: "paragraph", content: [{ type: "text", text: "kept " }, { type: "text", text: " old" }] }],
    });
  });

  it("mints tracked-change attributes with stable shape", () => {
    const attrs = newTrackedChangeAttrs("insert", "Alice");
    expect(attrs.kind).toBe("insert");
    expect(attrs.author).toBe("Alice");
    expect(attrs.changeId).toMatch(/^change-\d+-[a-z0-9]+$/);
    expect(typeof attrs.createdAt).toBe("string");
    const other = newTrackedChangeAttrs("insert", "Alice");
    expect(attrs.changeId).not.toBe(other.changeId);
  });

  it("derives inserted ranges from replace-step maps", () => {
    // Fake ReplaceStep: replaces 5..5 with 3 tokens (insertion at cursor).
    const insertStep = {
      slice: { size: 3 },
      getMap: () => ({
        forEach: (fn: (a: number, b: number, c: number, d: number) => void) => fn(5, -1, 5, 8),
      }),
    };
    // Fake deletion step: replaces 2..6 with nothing (collapsed target).
    const deleteStep = {
      slice: { size: 0 },
      getMap: () => ({
        forEach: (fn: (a: number, b: number, c: number, d: number) => void) => fn(2, 6, 2, 2),
      }),
    };
    const tr = { steps: [insertStep, deleteStep] };
    expect(insertedRangesFromTransaction(tr)).toEqual([{ from: 5, to: 8 }]);
  });

  it("ignores transactions whose steps carry no slice", () => {
    const tr = { steps: [{ getMap: () => ({ forEach: () => {} }) }] };
    expect(insertedRangesFromTransaction(tr)).toEqual([]);
  });

  it("derives deleted ranges from replace-step maps and keeps text-bearing spans only", () => {
    const deleteStep = {
      slice: { size: 0 },
      getMap: () => ({
        forEach: (fn: (a: number, b: number, c: number, d: number) => void) => fn(2, 6, 2, 2),
      }),
    };
    const textOldDoc = {
      nodesBetween: (_from: number, _to: number, fn: (node: any, _pos: number) => void) => {
        fn({ isText: true, text: "word" }, 3);
      },
    };
    expect(deletedRangesFromTransaction({ steps: [deleteStep] }, textOldDoc)).toEqual([
      { from: 2, to: 6 },
    ]);
    // Deletions that contained no text (e.g. block-level removals) are skipped.
    const blockOldDoc = {
      nodesBetween: (_from: number, _to: number, fn: (node: any, _pos: number) => void) => {
        fn({ isText: false, isBlock: true }, 2);
      },
    };
    expect(deletedRangesFromTransaction({ steps: [deleteStep] }, blockOldDoc)).toEqual([]);
  });
});
