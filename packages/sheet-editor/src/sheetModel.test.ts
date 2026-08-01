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
    chartType: makeAccessor<"bar" | "line" | "pie" | null>(null),
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
      "1,0": { raw: "=SUM(A1)", display: "10" },
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
    });
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
