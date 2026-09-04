import { describe, expect, it } from "vitest";
import { MAX_BOOKMARK_NAME_CHARS, findBookmarkPosition, normalizeBookmarkName } from "./bookmarks";
import { mySchema } from "./schema";

describe("bookmark names", () => {
  it("normalizes Word-compatible names deterministically", () => {
    expect(normalizeBookmarkName("  Project plan  ")).toBe("Project_plan");
    expect(normalizeBookmarkName("123 milestones")).toBe("_23_milestones");
    expect(normalizeBookmarkName("Road-map/2026")).toBe("Road_map_2026");
  });

  it("bounds names and rejects empty input", () => {
    expect(normalizeBookmarkName("   ")).toBeNull();
    expect(normalizeBookmarkName("a".repeat(MAX_BOOKMARK_NAME_CHARS + 8))).toHaveLength(MAX_BOOKMARK_NAME_CHARS);
  });

  it("finds an anchor by name without exposing the underlying mark layout", () => {
    const doc = mySchema.nodeFromJSON({
      type: "doc",
      content: [{ type: "paragraph", content: [{ type: "text", text: "Target", marks: [{ type: "bookmark", attrs: { name: "Target" } }] }] }],
    });
    expect(findBookmarkPosition(doc, "target")).toBe(1);
    expect(findBookmarkPosition(doc, "missing")).toBeNull();
  });
});
