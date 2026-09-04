import { describe, expect, it } from "vitest";
import {
  createSlicer,
  rowMatchesSlicers,
  slicerValues,
  toggleSlicerValue,
} from "./slicers";
import type { GridCell, SlicerConfig } from "./sheetTypes";

const cells: Record<string, GridCell> = {
  "1:1": { raw: "Region", display: "Region" },
  "1:2": { raw: "Sales", display: "Sales" },
  "2:1": { raw: "West", display: "West" },
  "2:2": { raw: "10", display: "10" },
  "3:1": { raw: "East", display: "East" },
  "3:2": { raw: "20", display: "20" },
  "4:1": { raw: "West", display: "West" },
  "4:2": { raw: "30", display: "30" },
};

describe("slicers", () => {
  it("creates bounded slicers and returns deterministic values", () => {
    const slicer = createSlicer(
      cells,
      { startRow: 1, endRow: 4, startCol: 1, endCol: 2 },
      1,
      "Sales regions",
      "slicer-1",
    );
    expect(slicer.title).toBe("Sales regions");
    expect(slicer.selectedValues).toEqual([]);
    expect(slicerValues(cells, slicer)).toEqual(["East", "West"]);
  });

  it("toggles from the implicit all-values state and collapses back to all", () => {
    const slicer: SlicerConfig = {
      id: "slicer-1",
      title: "Region",
      sourceRange: { startRow: 1, endRow: 4, startCol: 1, endCol: 2 },
      column: 1,
      selectedValues: [],
    };
    const allValues = ["East", "West"];
    const onlyEast = toggleSlicerValue(slicer, "West", allValues);
    expect(onlyEast.selectedValues).toEqual(["East"]);
    expect(rowMatchesSlicers(cells, 2, [onlyEast])).toBe(false);
    expect(rowMatchesSlicers(cells, 3, [onlyEast])).toBe(true);
    expect(toggleSlicerValue(onlyEast, "West", allValues).selectedValues).toEqual([]);
  });

  it("rejects invalid source columns and ranges", () => {
    expect(() => createSlicer(cells, { startRow: 1, endRow: 1, startCol: 1, endCol: 2 }, 1, "", "slicer-1"))
      .toThrow("header and at least one data row");
    expect(() => createSlicer(cells, { startRow: 1, endRow: 4, startCol: 1, endCol: 2 }, 3, "Region", "slicer-1"))
      .toThrow("inside the source range");
  });
});
