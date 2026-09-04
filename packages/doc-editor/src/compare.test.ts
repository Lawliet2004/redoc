import { describe, expect, it } from "vitest";
import { compareDocContent, findCompareSource } from "./compare";

const doc = (text: string) => ({
  type: "doc",
  content: [{ type: "paragraph", content: [{ type: "text", text }] }],
});

describe("document compare", () => {
  it("resolves only a selected non-empty open-document source", () => {
    const source = { id: "two", title: "Second", content: doc("Other") };
    expect(findCompareSource([source], "two")).toBe(source);
    expect(findCompareSource([{ id: "empty", title: "Empty", content: null }], "empty")).toBeNull();
    expect(findCompareSource([source], "")).toBeNull();
  });

  it("returns stable word-level insertions and deletions", () => {
    const result = compareDocContent(doc("Hello brave world"), doc("Hello kind world"));
    expect(result.changed).toBe(true);
    expect(result.insertedWords).toBe(1);
    expect(result.deletedWords).toBe(1);
    expect(result.segments).toEqual([
      { kind: "equal", text: "Hello " },
      { kind: "delete", text: "brave" },
      { kind: "insert", text: "kind" },
      { kind: "equal", text: " world" },
    ]);
  });

  it("includes block boundaries without duplicating text", () => {
    const result = compareDocContent(
      { type: "doc", content: [{ type: "paragraph", content: [{ type: "text", text: "One" }] }, { type: "paragraph", content: [{ type: "text", text: "Two" }] }] },
      { type: "doc", content: [{ type: "paragraph", content: [{ type: "text", text: "One" }] }, { type: "paragraph", content: [{ type: "text", text: "Three" }] }] },
    );
    expect(result.beforeText).toBe("One\nTwo");
    expect(result.afterText).toBe("One\nThree");
  });

  it("uses a bounded fallback for very large documents", () => {
    const before = doc(Array.from({ length: 2_001 }, (_, index) => `word${index}`).join(" "));
    const after = doc(Array.from({ length: 2_001 }, (_, index) => `word${index === 1_000 ? "changed" : index}`).join(" "));
    const result = compareDocContent(before, after);
    expect(result.truncated).toBe(true);
    expect(result.changed).toBe(true);
    expect(result.insertedWords).toBe(1);
    expect(result.deletedWords).toBe(1);
  });
});
