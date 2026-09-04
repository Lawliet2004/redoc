import { describe, expect, it } from "vitest";
import {
  collectFootnoteRefIds,
  readFootnotes,
  syncFootnotesWithRefs,
  type Footnote,
} from "./footnotes";

const doc = (body: Array<Record<string, unknown>>) => ({
  type: "doc",
  content: [{ type: "paragraph", content: body }],
});

describe("footnote persistence helpers", () => {
  it("reads bounded footnotes from a doc", () => {
    const notes = readFootnotes({
      type: "doc",
      footnotes: [
        { id: "a", label: 3, text: "one" },
        { id: "b", label: 0, text: "two" },
        { id: "", text: "dropped" },
        "junk",
      ],
    } as never);
    expect(notes).toHaveLength(2);
    expect(notes[0]).toEqual({ id: "a", label: 3, text: "one" });
    expect(notes[1].label).toBe(1); // invalid 0 label clamps to 1
  });

  it("collects referenced ids recursively", () => {
    const ids = collectFootnoteRefIds(
      doc([
        { type: "text", text: "hi" },
        { type: "footnote_ref", attrs: { id: "x" } },
      ]) as never,
    );
    expect([...ids]).toEqual(["x"]);
  });

  it("renumbers notes in body reference order and keeps orphans", () => {
    const notes: Footnote[] = [
      { id: "n2", label: 1, text: "second" },
      { id: "n1", label: 2, text: "first" },
      { id: "orphan", label: 3, text: "kept text" },
    ];
    const body = doc([
      { type: "footnote_ref", attrs: { id: "n1" } },
      { type: "footnote_ref", attrs: { id: "n2" } },
    ]);
    const synced = syncFootnotesWithRefs(notes, body as never);
    expect(synced.map((n) => n.id)).toEqual(["n1", "n2", "orphan"]);
    expect(synced.map((n) => n.label)).toEqual([1, 2, 3]);
  });

  it("drops empty notes when nothing references them", () => {
    const synced = syncFootnotesWithRefs(
      [{ id: "empty", label: 1, text: "   " }],
      doc([{ type: "text", text: "no refs" }]) as never,
    );
    expect(synced).toHaveLength(0);
  });
});
