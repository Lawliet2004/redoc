import { describe, expect, it } from "vitest";
import { buildPivotTable, type PivotTableConfig } from "./pivotTables";
import type { GridCell } from "./sheetTypes";

const cellMap = (rows: string[][]): Record<string, GridCell> => Object.fromEntries(
  rows.flatMap((row, rowIndex) => row.map((value, colIndex) => [
    `${rowIndex + 1}:${colIndex + 1}`,
    { raw: value, display: value },
  ])),
);

const baseConfig: PivotTableConfig = {
  id: "pivot-1",
  sourceRange: { startRow: 1, endRow: 5, startCol: 1, endCol: 2 },
  rowField: 1,
  valueField: 2,
  aggregation: "sum",
  outputStartRow: 1,
  outputStartCol: 4,
};

describe("pivot tables", () => {
  it("groups repeated row labels and calculates sums", () => {
    const result = buildPivotTable(cellMap([
      ["Team", "Hours"],
      ["A", "2"],
      ["B", "3"],
      ["A", "4"],
      ["B", "1"],
    ]), baseConfig);
    expect(result.warning).toBeUndefined();
    expect(result.cells["1:4"].display).toBe("Team");
    expect(result.cells["2:4"].display).toBe("A");
    expect(result.cells["2:5"].display).toBe("6");
    expect(result.cells["3:5"].display).toBe("4");
    expect(result.cells["4:4"].display).toBe("Grand Total");
  });

  it("supports count and average without turning non-numeric values into zero sums", () => {
    const cells = cellMap([
      ["Team", "Hours"],
      ["A", "2"],
      ["A", "n/a"],
      ["B", "4"],
    ]);
    expect(buildPivotTable(cells, { ...baseConfig, sourceRange: { ...baseConfig.sourceRange, endRow: 4 }, aggregation: "count" }).cells["2:5"].display).toBe("2");
    const average = buildPivotTable(cells, { ...baseConfig, sourceRange: { ...baseConfig.sourceRange, endRow: 4 }, aggregation: "average" });
    expect(average.cells["2:5"].display).toBe("2.00");
    expect(average.cells["4:5"].display).toBe("3.00");
  });

  it("rejects invalid field and source ranges with a warning", () => {
    const result = buildPivotTable(cellMap([["A"]]), { ...baseConfig, valueField: 3 });
    expect(result.cells).toEqual({});
    expect(result.warning).toContain("inside the source range");
  });
});
