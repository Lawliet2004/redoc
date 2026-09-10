import { describe, expect, it } from "vitest";
import { mergeTableCells, splitTableCell, isCoveredByMerge, mergeAt, nextTableStyle } from "./shapeUtils";
import type { TableMerge } from "./deckNormalize";

const data = [
  ["a", "b", "c"],
  ["d", "e", "f"],
  ["g", "h", "i"],
];

describe("mergeTableCells", () => {
  it("creates a bounded merge and rejects overlaps", () => {
    const first = mergeTableCells(data, [], 0, 0, 1, 2);
    expect(first).toEqual([{ r: 0, c: 0, rowspan: 1, colspan: 2 }]);
    expect(mergeTableCells(data, first!, 0, 1, 1, 2)).toBeNull();
    expect(mergeTableCells(data, first!, 1, 0, 2, 1)).toEqual([
      ...first!,
      { r: 1, c: 0, rowspan: 2, colspan: 1 },
    ]);
  });

  it("clamps spans to the grid and rejects single cells", () => {
    expect(mergeTableCells(data, [], 1, 1, 99, 99)).toEqual([{ r: 1, c: 1, rowspan: 2, colspan: 2 }]);
    expect(mergeTableCells(data, [], 0, 0, 1, 1)).toBeNull();
  });
});

describe("splitTableCell and coverage", () => {
  it("splits the merge covering a cell and keeps others", () => {
    const merges: TableMerge[] = [
      { r: 0, c: 0, rowspan: 1, colspan: 2 },
      { r: 1, c: 1, rowspan: 2, colspan: 1 },
    ];
    expect(splitTableCell(merges, 0, 1)).toEqual([{ r: 1, c: 1, rowspan: 2, colspan: 1 }]);
    expect(isCoveredByMerge(merges, 0, 1)).toBe(true);
    expect(isCoveredByMerge(merges, 0, 0)).toBe(false);
    expect(mergeAt(merges, 2, 1)).toEqual({ r: 1, c: 1, rowspan: 2, colspan: 1 });
    expect(mergeAt(merges, 2, 2)).toBeNull();
  });
});

describe("nextTableStyle", () => {
  it("cycles plain, banded, accent-header", () => {
    expect(nextTableStyle("plain")).toBe("banded");
    expect(nextTableStyle("banded")).toBe("accent-header");
    expect(nextTableStyle("accent-header")).toBe("plain");
    expect(nextTableStyle(undefined)).toBe("plain");
  });
});
