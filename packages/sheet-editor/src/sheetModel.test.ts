import { describe, expect, it } from "vitest";
import { parseTsv, serializeTsv } from "@redoc/utils";
import { buildWorkbookFromCells } from "./sheetModel";
import type { GridCell } from "./sheetTypes";

function makeAccessor<T>(value: T): () => T {
  return () => value;
}

function makeSetter<T>(): { value: T; setter: (next: T | ((prev: T) => T)) => void } {
  let value: T;
  return {
    get value() {
      return value!;
    },
    set value(v: T) {
      value = v;
    },
    setter(next: T | ((prev: T) => T)) {
      value = typeof next === "function" ? (next as (prev: T) => T)(value) : next;
    },
  };
}

/** A fresh per-sheet data cache backed by a mutable record. */
function createCache(): { value: Record<number, any> } {
  const cache = makeSetter<Record<number, any>>();
  cache.value = {};
  return cache;
}

function buildMinimalContext(overrides: Partial<Parameters<typeof buildWorkbookFromCells>[0]> = {}) {
  const cache = makeSetter<Record<number, any>>();
  cache.value = {};

  return {
    initialContent: undefined,
    sheets: makeAccessor([{ id: "sheet-1", name: "Sheet1" }]),
    sheetDataCache: makeAccessor(cache.value),
    setSheetDataCache: cache.setter,
    activeSheetIndex: makeAccessor(0),
    freezeRows: makeAccessor(1),
    freezeCols: makeAccessor(2),
    columnWidth: makeAccessor(100),
    rowHeight: makeAccessor(24),
    columnWidths: makeAccessor({ 3: 140 }),
    rowHeights: makeAccessor({ 4: 32 }),
    selectionAnchor: makeAccessor({ row: 1, col: 1 }),
    activeCell: makeAccessor({ row: 2, col: 3 }),
    chartType: makeAccessor<"bar" | "line" | "pie" | "area" | "scatter" | "doughnut" | null>(null),
    chartTitle: makeAccessor(""),
    chartRange: makeAccessor({ startRow: 0, endRow: 0, startCol: 0, endCol: 0 }),
    filterQuery: makeAccessor(""),
    merges: makeAccessor([]),
    autoFilterEnabled: makeAccessor(false),
    filterRange: makeAccessor(null),
    namedRanges: makeAccessor([]),
    columnFilters: makeAccessor({}),
    ...overrides,
  };
}

describe("buildWorkbookFromCells", () => {
  it("produces expected sheet structure from cell map", () => {
    const ctx = buildMinimalContext();
    const cellMap: Record<string, GridCell> = {
      "0,0": { raw: "Hello", display: "Hello", style: { bold: true } },
      "1,0": { raw: "=SUM(A1)", display: "10", style: { image: "data:image/png;base64,aA==" } },
    };

    const workbook = buildWorkbookFromCells(ctx, cellMap);

    expect(workbook.activeSheetIndex).toBe(0);
    expect(workbook.sheets).toHaveLength(1);
    const sheet = workbook.sheets[0];
    expect(sheet.id).toBe("sheet-1");
    expect(sheet.name).toBe("Sheet1");
    expect(sheet.freezeRows).toBe(1);
    expect(sheet.freezeCols).toBe(2);
    expect(sheet.colWidths).toEqual({ 0: 100, 3: 140 });
    expect(sheet.rowHeights).toEqual({ 0: 24, 4: 32 });
    expect(sheet.cells["0,0"]).toMatchObject({
      rawValue: "Hello",
      displayValue: "Hello",
      formula: null,
      style: { bold: true },
    });
    expect(sheet.cells["1,0"]).toMatchObject({
      rawValue: "=SUM(A1)",
      formula: "=SUM(A1)",
      style: { image: "data:image/png;base64,aA==" },
    });
  });

  it("persists the six designer chart types through workbook builds", () => {
    for (const type of ["bar", "line", "area", "scatter", "pie", "doughnut"] as const) {
      const ctx = buildMinimalContext({
        chartType: makeAccessor(type),
        chartTitle: makeAccessor(`Chart ${type}`),
      });
      const workbook = buildWorkbookFromCells(ctx, {});
      expect(workbook.sheets[0].charts).toHaveLength(1);
      expect(workbook.sheets[0].charts[0].chartType).toBe(type);
      expect(workbook.sheets[0].charts[0].title).toBe(`Chart ${type}`);
    }
    const cleared = buildMinimalContext({ chartType: makeAccessor(null) });
    expect(buildWorkbookFromCells(cleared, {}).sheets[0].charts).toEqual([]);
  });

  it("preserves previously saved charts when no chart is active in the context", () => {
    const cache = createCache();
    const initialContent = {
      activeSheetIndex: 0,
      sheets: [
        {
          id: "sheet-1",
          name: "Sheet1",
          cells: {},
          colWidths: {},
          rowHeights: {},
          freezeRows: 0,
          freezeCols: 0,
          charts: [
            { chartType: "pie", title: "First", startRow: 1, endRow: 3, startCol: 1, endCol: 2 },
            { chartType: "line", title: "Second", startRow: 4, endRow: 6, startCol: 1, endCol: 2 },
          ],
          conditionalFormatting: [],
          spills: [],
        },
      ],
    };
    // Context has no chart active (user re-opened the sheet, did not insert).
    const ctx = buildMinimalContext({ initialContent, sheetDataCache: makeAccessor(cache.value) });
    const workbook = buildWorkbookFromCells(ctx, {});
    const charts = workbook.sheets[0].charts;
    expect(charts).toHaveLength(2);
    expect(charts[0].chartType).toBe("pie");
    expect(charts[1].chartType).toBe("line");
  });

  it("replaces only the first chart slot when one is being edited", () => {
    const cache = createCache();
    const initialContent = {
      activeSheetIndex: 0,
      sheets: [
        {
          id: "sheet-1",
          name: "Sheet1",
          cells: {},
          colWidths: {},
          rowHeights: {},
          freezeRows: 0,
          freezeCols: 0,
          charts: [
            { chartType: "pie", title: "First", startRow: 1, endRow: 3, startCol: 1, endCol: 2 },
            { chartType: "line", title: "Second", startRow: 4, endRow: 6, startCol: 1, endCol: 2 },
          ],
          conditionalFormatting: [],
          spills: [],
        },
      ],
    };
    const ctx = buildMinimalContext({
      initialContent,
      sheetDataCache: makeAccessor(cache.value),
      chartType: makeAccessor<"bar" | "line" | "pie" | "area" | "scatter" | "doughnut" | null>("bar"),
      chartTitle: makeAccessor("Edited"),
      chartRange: makeAccessor({ startRow: 9, endRow: 12, startCol: 1, endCol: 2 }),
    });
    const workbook = buildWorkbookFromCells(ctx, {});
    const charts = workbook.sheets[0].charts;
    expect(charts).toHaveLength(2);
    expect(charts[0].title).toBe("Edited");
    expect(charts[1].chartType).toBe("line");
  });

  it("materializes persisted pivot summaries during workbook builds", () => {
    const ctx = buildMinimalContext({
      pivotTables: makeAccessor([{
        id: "pivot-1",
        sourceRange: { startRow: 1, endRow: 3, startCol: 1, endCol: 2 },
        rowField: 1,
        valueField: 2,
        aggregation: "sum",
        outputStartRow: 5,
        outputStartCol: 1,
      }]),
    });
    const workbook = buildWorkbookFromCells(ctx, {
      "1:1": { raw: "Team", display: "Team" },
      "1:2": { raw: "Hours", display: "Hours" },
      "2:1": { raw: "A", display: "A" },
      "2:2": { raw: "2", display: "2" },
      "3:1": { raw: "A", display: "A" },
      "3:2": { raw: "3", display: "3" },
    });
    expect(workbook.sheets[0].pivotTables[0].outputRowCount).toBe(3);
    expect(workbook.sheets[0].cells["6:2"].displayValue).toBe("5");
  });

  it("refreshes persisted pivots through active slicer selections", () => {
    const ctx = buildMinimalContext({
      pivotTables: makeAccessor([{
        id: "pivot-1",
        sourceRange: { startRow: 1, endRow: 4, startCol: 1, endCol: 2 },
        rowField: 1,
        valueField: 2,
        aggregation: "sum",
        outputStartRow: 6,
        outputStartCol: 1,
      }]),
      slicers: makeAccessor([{
        id: "slicer-1",
        title: "Region",
        sourceRange: { startRow: 1, endRow: 4, startCol: 1, endCol: 2 },
        column: 1,
        selectedValues: ["East"],
      }]),
    });
    const workbook = buildWorkbookFromCells(ctx, {
      "1:1": { raw: "Region", display: "Region" },
      "1:2": { raw: "Sales", display: "Sales" },
      "2:1": { raw: "West", display: "West" },
      "2:2": { raw: "10", display: "10" },
      "3:1": { raw: "East", display: "East" },
      "3:2": { raw: "20", display: "20" },
      "4:1": { raw: "West", display: "West" },
      "4:2": { raw: "30", display: "30" },
    });
    expect(workbook.sheets[0].cells["7:1"].displayValue).toBe("East");
    expect(workbook.sheets[0].cells["7:2"].displayValue).toBe("20");
    expect(workbook.sheets[0].pivotTables[0].outputRowCount).toBe(3);
    expect(workbook.sheets[0].cells["8:1"].displayValue).toBe("Grand Total");
    expect(workbook.sheets[0].cells["8:2"].displayValue).toBe("20");
  });

  it("preserves imported table metadata while editing cells", () => {
    const ctx = buildMinimalContext({
      initialContent: {
        activeSheetIndex: 0,
        sheets: [{
          id: "sheet-1",
          name: "Sheet1",
          cells: {},
          colWidths: {},
          rowHeights: {},
          freezeRows: 0,
          freezeCols: 0,
          tables: [{
            id: "1",
            name: "SalesTable",
            range: { startRow: 1, endRow: 3, startCol: 1, endCol: 2 },
            columns: ["Region", "Sales"],
            style: "TableStyleMedium2",
            showHeaderRow: true,
            showTotalRow: false,
          }],
        }],
      },
    });
    const workbook = buildWorkbookFromCells(ctx, {
      "1:1": { raw: "Region", display: "Region" },
    });
    expect(workbook.sheets[0].tables).toEqual([expect.objectContaining({
      name: "SalesTable",
      columns: ["Region", "Sales"],
    })]);
  });

  it("preserves saved what-if scenarios while editing cells", () => {
    const ctx = buildMinimalContext({
      scenarios: makeAccessor([{
        id: "scenario-1",
        name: "Higher demand",
        changes: [{ row: 2, col: 2, rawValue: "125" }],
      }]),
    });
    const workbook = buildWorkbookFromCells(ctx, {
      "2:2": { raw: "100", display: "100" },
    });
    expect(workbook.sheets[0].scenarios).toEqual([expect.objectContaining({
      id: "scenario-1",
      name: "Higher demand",
      changes: [{ row: 2, col: 2, rawValue: "125" }],
    })]);
  });

  it("preserves saved slicers while editing cells", () => {
    const ctx = buildMinimalContext({
      slicers: makeAccessor([{
        id: "slicer-1",
        title: "Region",
        sourceRange: { startRow: 1, endRow: 3, startCol: 1, endCol: 2 },
        column: 1,
        selectedValues: ["East"],
      }]),
    });
    const workbook = buildWorkbookFromCells(ctx, {
      "1:1": { raw: "Region", display: "Region" },
    });
    expect(workbook.sheets[0].slicers).toEqual([expect.objectContaining({
      id: "slicer-1",
      title: "Region",
      selectedValues: ["East"],
    })]);
  });

  it("keeps spill metadata while excluding synthetic spill cells", () => {
    const ctx = buildMinimalContext({
      initialContent: {
        activeSheetIndex: 0,
        sheets: [{
          id: "sheet-1",
          name: "Sheet1",
          cells: {},
          colWidths: {},
          rowHeights: {},
          freezeRows: 0,
          freezeCols: 0,
          spills: [{ originRow: 1, originCol: 2, rows: 2, cols: 1, values: ["A", "B"] }],
        }],
      },
    });
    const workbook = buildWorkbookFromCells(ctx, {
      "1:2": { raw: "=TRANSPOSE(A1:A2)", display: "A" },
      "2:2": { raw: "", display: "B", spill: true },
    });
    expect(workbook.sheets[0].spills).toEqual([
      { originRow: 1, originCol: 2, rows: 2, cols: 1, values: ["A", "B"] },
    ]);
    expect(workbook.sheets[0].cells["2:2"]).toBeUndefined();
    expect(workbook.sheets[0].cells["1:2"]).toMatchObject({ formula: "=TRANSPOSE(A1:A2)" });
  });
});

describe("TSV round-trip via @redoc/utils", () => {
  it("parseTsv and serializeTsv round-trip sheet clipboard data", () => {
    const data = [
      ["Name", "Qty", "Price"],
      ["Widget", "3", "9.99"],
      ["Gadget", "1", "4.50"],
    ];
    const tsv = serializeTsv(data);
    expect(parseTsv(tsv)).toEqual(data);
  });
});
