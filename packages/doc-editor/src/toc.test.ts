import { describe, it, expect } from "vitest";
import { buildTocEntries, buildTocContent } from "./toc";
import type { DocContent } from "./types";

const doc: DocContent = {
  type: "doc",
  content: [
    { type: "heading", attrs: { level: 1 }, content: [{ type: "text", text: "Introduction" }] },
    { type: "paragraph", content: [{ type: "text", text: "Body text" }] },
    { type: "heading", attrs: { level: 2 }, content: [{ type: "text", text: "Methods & Data" }] },
    { type: "heading", attrs: { level: 1 }, content: [{ type: "text", text: "Results" }] },
    { type: "heading", attrs: { level: 9 }, content: [{ type: "text", text: "Deep" }] },
  ],
};

describe("buildTocEntries", () => {
  it("collects headings in order with levels", () => {
    const entries = buildTocEntries(doc);
    expect(entries.map((e) => [e.level, e.text])).toEqual([
      [1, "Introduction"],
      [2, "Methods & Data"],
      [1, "Results"],
      [6, "Deep"],
    ]);
  });

  it("skips empty-text headings", () => {
    const empty: DocContent = {
      type: "doc",
      content: [{ type: "heading", attrs: { level: 1 }, content: [] }],
    };
    expect(buildTocEntries(empty)).toEqual([]);
  });

  it("sanitizes anchor names and deduplicates them", () => {
    const entries = buildTocEntries(doc);
    expect(entries[0].anchor).toBe("Introduction");
    expect(entries[1].anchor).toBe("Methods_Data");
    const dup = buildTocEntries({
      type: "doc",
      content: [
        { type: "heading", attrs: { level: 1 }, content: [{ type: "text", text: "Same" }] },
        { type: "heading", attrs: { level: 1 }, content: [{ type: "text", text: "Same" }] },
      ],
    });
    expect(dup[0].anchor).not.toBe(dup[1].anchor);
  });

  it("returns empty for null/undefined documents", () => {
    expect(buildTocEntries(null)).toEqual([]);
    expect(buildTocEntries(undefined)).toEqual([]);
  });

  it("caps the number of entries", () => {
    const many: DocContent = {
      type: "doc",
      content: Array.from({ length: 300 }, (_, i) => ({
        type: "heading",
        attrs: { level: 1 },
        content: [{ type: "text", text: `H${i}` }],
      })),
    };
    expect(buildTocEntries(many).length).toBeLessThanOrEqual(200);
  });
});

describe("buildTocContent", () => {
  it("renders indented internal-link paragraphs", () => {
    const entries = buildTocEntries(doc);
    const content = buildTocContent(entries);
    expect(content).toHaveLength(4);
    expect(content[0].type).toBe("paragraph");
    expect(content[0].attrs?.indent).toBe(0);
    expect(content[1].attrs?.indent).toBe(0.75);
    const text = content[0].content![0] as DocContent;
    expect(text.marks?.[0].type).toBe("link");
    expect(text.marks?.[0].attrs?.href).toBe("internal:Introduction");
  });

  it("produces no content for no entries", () => {
    expect(buildTocContent([])).toEqual([]);
  });
});
