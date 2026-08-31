import { describe, expect, it } from "vitest";
import { getTrackedChangeStats, resolveTrackedChanges } from "./trackedChanges";

const doc = {
  type: "doc",
  content: [{
    type: "paragraph",
    content: [
      { type: "text", text: "kept " },
      { type: "text", text: "new", marks: [{ type: "trackInsert", attrs: { author: "You" } }] },
      { type: "text", text: " old", marks: [{ type: "trackDelete", attrs: { author: "You" } }] },
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

  it("rejects insertions and clears surviving deletion marks", () => {
    expect(resolveTrackedChanges(doc, "reject")).toEqual({
      type: "doc",
      content: [{ type: "paragraph", content: [{ type: "text", text: "kept " }, { type: "text", text: " old" }] }],
    });
  });
});
