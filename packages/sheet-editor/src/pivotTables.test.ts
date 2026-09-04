import { describe, expect, it } from "vitest";
import { buildPivotTable, buildMultiFieldPivotTable, type PivotTableConfig } from "./pivotTables";
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

  it("refreshes from a bounded visible-row predicate", () => {
    const result = buildPivotTable(
      cellMap([
        ["Team", "Hours"],
        ["A", "2"],
        ["B", "3"],
        ["A", "4"],
      ]),
      { ...baseConfig, sourceRange: { ...baseConfig.sourceRange, endRow: 4 } },
      (row) => row !== 3,
    );
    expect(result.cells["2:4"].display).toBe("A");
    expect(result.cells["2:5"].display).toBe("6");
    expect(result.cells["3:4"].display).toBe("Grand Total");
    expect(result.cells["3:5"].display).toBe("6");
  });

  it("rejects invalid field and source ranges with a warning", () => {
    const result = buildPivotTable(cellMap([["A"]]), { ...baseConfig, valueField: 3 });
    expect(result.cells).toEqual({});
    expect(result.warning).toContain("inside the source range");
  });
});

describe("multi-field pivot tables", () => {
  const multiConfig = {
    sourceRange: { startRow: 1, endRow: 7, startCol: 1, endCol: 3 },
    rowField: 1,
    columnField: 2,
    valueField: 3,
    aggregation: "sum" as const,
    outputStartRow: 10,
    outputStartCol: 1,
  };

  const source = cellMap([
    ["Region", "Quarter", "Sales"],
    ["East", "Q1", "10"],
    ["East", "Q2", "20"],
    ["West", "Q1", "5"],
    ["West", "Q2", "15"],
    ["East", "Q1", "30"],
    ["West", "Q2", "25"],
  ]);

  it("crosses rows by columns with sums and grand totals", () => {
    const result = buildMultiFieldPivotTable(source, multiConfig);
    expect(result.warning).toBeUndefined();
    expect(result.cells["10:1"].display).toContain("Sum of Sales");
    expect(result.cells["10:2"].display).toBe("Q1");
    expect(result.cells["10:3"].display).toBe("Q2");
    expect(result.cells["10:4"].display).toBe("Grand Total");
    // East row: Q1 = 10+30 = 40, Q2 = 20, total 60
    expect(result.cells["11:1"].display).toBe("East");
    expect(result.cells["11:2"].display).toBe("40");
    expect(result.cells["11:3"].display).toBe("20");
    expect(result.cells["11:4"].display).toBe("60");
    // West row: Q1 = 5, Q2 = 15+25 = 40, total 45
    expect(result.cells["12:2"].display).toBe("5");
    expect(result.cells["12:3"].display).toBe("40");
    expect(result.cells["12:4"].display).toBe("45");
    // Grand total row: Q1 = 45, Q2 = 60, all = 105
    expect(result.cells["13:1"].display).toBe("Grand Total");
    expect(result.cells["13:2"].display).toBe("45");
    expect(result.cells["13:3"].display).toBe("60");
    expect(result.cells["13:4"].display).toBe("105");
    expect(result.rowCount).toBe(4);
    expect(result.colCount).toBe(4);
  });

  it("supports count and average aggregations", () => {
    const count = buildMultiFieldPivotTable(source, { ...multiConfig, aggregation: "count" });
    // East/Q1 has 2 entries; count only counts non-empty values.
    expect(count.cells["11:2"].display).toBe("2");
    const average = buildMultiFieldPivotTable(source, { ...multiConfig, aggregation: "average" });
    expect(average.cells["11:2"].display).toBe("20.00");
    expect(average.cells["11:4"].display).toBe("20.00");
  });

  it("applies the visible-row predicate (slicer filtering)", () => {
    const result = buildMultiFieldPivotTable(
      source,
      multiConfig,
      (row) => row !== 6 && row !== 7,
    );
    // Excluding rows 6 (East Q1 30) and 7 (West Q2 25): East Q1 = 10, West Q2 = 15.
    expect(result.cells["11:2"].display).toBe("10");
    expect(result.cells["12:3"].display).toBe("15");
    expect(result.cells["13:4"].display).toBe("50");
  });

  it("rejects identical row and column fields", () => {
    const result = buildMultiFieldPivotTable(source, { ...multiConfig, columnField: 1 });
    expect(result.warning).toContain("must differ");
  });

  it("rejects fields outside the source range", () => {
    const result = buildMultiFieldPivotTable(source, { ...multiConfig, valueField: 9 });
    expect(result.warning).toContain("inside the source range");
  });

  it("warns when the group bound is exceeded", () => {
    const wide = cellMap([
      ["Region", "Quarter", "Sales"],
      ...Array.from({ length: 501 }, () => ["R", `Q${Math.random()}`, "1"] as string[]),
    ]);
    const result = buildMultiFieldPivotTable(wide, { ...multiConfig, sourceRange: { startRow: 1, endRow: 502, startCol: 1, endCol: 3 } });
    expect(result.warning).toContain("group bound");
  });
});
