import type { Accessor, Setter } from "solid-js";
import type { GridCell, SheetSnapshot } from "./sheetTypes";

export interface SheetHistoryOptions {
  historyPast: Accessor<SheetSnapshot[]>;
  setHistoryPast: Setter<SheetSnapshot[]>;
  historyFuture: Accessor<SheetSnapshot[]>;
  setHistoryFuture: Setter<SheetSnapshot[]>;
  captureSnapshot: () => SheetSnapshot;
  restoreSnapshot: (snap: SheetSnapshot) => Promise<void>;
}

export function createSheetHistoryHandlers(opts: SheetHistoryOptions) {
  const pushHistory = () => {
    opts.setHistoryPast([...opts.historyPast(), opts.captureSnapshot()].slice(-200));
    opts.setHistoryFuture([]);
  };

  const undo = async () => {
    const past = opts.historyPast();
    const previous = past.at(-1);
    if (!previous) return;
    opts.setHistoryPast(past.slice(0, -1));
    opts.setHistoryFuture([...opts.historyFuture(), opts.captureSnapshot()].slice(-200));
    await opts.restoreSnapshot(previous);
  };

  const redo = async () => {
    const future = opts.historyFuture();
    const next = future.at(-1);
    if (!next) return;
    opts.setHistoryFuture(future.slice(0, -1));
    opts.setHistoryPast([...opts.historyPast(), opts.captureSnapshot()].slice(-200));
    await opts.restoreSnapshot(next);
  };

  return { pushHistory, undo, redo };
}

export interface BuildWorkbookContext {
  initialContent?: any;
  sheets: Accessor<Array<{ id: string; name: string }>>;
  sheetDataCache: Accessor<Record<number, any>>;
  setSheetDataCache: Setter<Record<number, any>>;
  activeSheetIndex: Accessor<number>;
  freezeRows: Accessor<number>;
  freezeCols: Accessor<number>;
  columnWidth: Accessor<number>;
  rowHeight: Accessor<number>;
  columnWidths: Accessor<Record<number, number>>;
  rowHeights: Accessor<Record<number, number>>;
  selectionAnchor: Accessor<{ row: number; col: number }>;
  activeCell: Accessor<{ row: number; col: number }>;
  chartType: Accessor<"bar" | "line" | "pie" | null>;
  chartTitle: Accessor<string>;
  chartRange: Accessor<{ startRow: number; endRow: number; startCol: number; endCol: number }>;
  filterQuery: Accessor<string>;
  merges: Accessor<import("./sheetTypes").MergeRange[]>;
  autoFilterEnabled: Accessor<boolean>;
  filterRange: Accessor<{ startRow: number; endRow: number; startCol: number; endCol: number } | null>;
  columnFilters: Accessor<Record<number, string[]>>;
  namedRanges: Accessor<Array<{ name: string; rangeStr: string; sheet: string | null }>>;
}

export function buildWorkbookFromCells(ctx: BuildWorkbookContext, cellMap: Record<string, GridCell>) {
  const existing = ctx.initialContent?.sheets?.length
    ? structuredClone(ctx.initialContent)
    : { sheets: [{ id: "sheet-1", name: "Sheet1", cells: {}, colWidths: {}, rowHeights: {}, freezeRows: 0, freezeCols: 0 }], activeSheetIndex: 0 };
  const currentMeta = ctx.sheets();
  if (existing.sheets) {
    existing.sheets = currentMeta.map((s, idx) => {
      const cached = ctx.sheetDataCache()[idx];
      const found = cached || existing.sheets?.find((es: any) => es.id === s.id) || existing.sheets?.[idx];
      return {
        id: s.id,
        name: s.name,
        cells: found?.cells || {},
        colWidths: found?.colWidths || {},
        rowHeights: found?.rowHeights || {},
        freezeRows: found?.freezeRows || 0,
        freezeCols: found?.freezeCols || 0,
        charts: found?.charts || [],
        filterQuery: found?.filterQuery || null,
        merges: found?.merges || [],
        autoFilter: found?.autoFilter || null,
      };
    });
  }
  const sheet = existing.sheets[ctx.activeSheetIndex()] || existing.sheets[0];
  sheet.freezeRows = ctx.freezeRows();
  sheet.freezeCols = ctx.freezeCols();
  sheet.colWidths = { 0: ctx.columnWidth(), ...ctx.columnWidths() };
  sheet.rowHeights = { 0: ctx.rowHeight(), ...ctx.rowHeights() };
  const anchor = ctx.selectionAnchor();
  const active = ctx.activeCell();
  sheet.charts = ctx.chartType()
    ? [{
        chartType: ctx.chartType(),
        title: ctx.chartTitle(),
        startRow: ctx.chartRange().startRow,
        endRow: ctx.chartRange().endRow,
        startCol: ctx.chartRange().startCol,
        endCol: ctx.chartRange().endCol,
      }]
    : [];
  sheet.filterQuery = ctx.filterQuery() || null;
  sheet.merges = ctx.merges();
  const fr = ctx.filterRange() || {
    startRow: Math.min(anchor.row, active.row),
    endRow: Math.max(anchor.row, active.row),
    startCol: Math.min(anchor.col, active.col),
    endCol: Math.max(anchor.col, active.col),
  };
  sheet.autoFilter = ctx.autoFilterEnabled()
    ? {
        enabled: true,
        startRow: fr.startRow,
        endRow: fr.endRow,
        startCol: fr.startCol,
        endCol: fr.endCol,
        columnFilters: Object.fromEntries(Object.entries(ctx.columnFilters()).map(([k, v]) => [k, v])),
      }
    : null;
  sheet.cells = Object.fromEntries(Object.entries(cellMap).map(([key, value]) => [key, {
    rawValue: value.raw,
    displayValue: value.display,
    formula: value.raw.startsWith("=") ? value.raw : null,
    style: value.style
      ? {
          bold: value.style.bold,
          italic: value.style.italic,
          underline: value.style.underline,
          fontColor: value.style.fontColor,
          bgColor: value.style.bgColor,
          align: value.style.align,
          format: value.style.format,
          decimals: value.style.decimals,
          hyperlink: value.style.hyperlink,
          validation: value.style.validation,
          wrap: value.style.wrap,
          vAlign: value.style.vAlign,
          fontFamily: value.style.fontFamily,
          fontSize: value.style.fontSize,
        }
      : null,
  }]));
  existing.activeSheetIndex = ctx.activeSheetIndex();
  existing.namedRanges = ctx.namedRanges();
  const cache: Record<number, any> = { ...ctx.sheetDataCache() };
  existing.sheets?.forEach((s: any, idx: number) => {
    cache[idx] = s;
  });
  ctx.setSheetDataCache(cache);
  return existing;
}
