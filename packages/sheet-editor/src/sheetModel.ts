import type { Accessor, Setter } from "solid-js";
import type { ConditionalFormattingRule, GridCell, PivotTableConfig, ScenarioConfig, SheetSnapshot, SlicerConfig } from "./sheetTypes";
import { buildPivotTable } from "./pivotTables";
import { rowMatchesSlicers } from "./slicers";

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
  sheets: Accessor<Array<{ id: string; name: string; color?: string }>>;
  sheetDataCache: Accessor<Record<string, any>>;
  setSheetDataCache: Setter<Record<string, any>>;
  activeSheetIndex: Accessor<number>;
  freezeRows: Accessor<number>;
  freezeCols: Accessor<number>;
  columnWidth: Accessor<number>;
  rowHeight: Accessor<number>;
  columnWidths: Accessor<Record<number, number>>;
  rowHeights: Accessor<Record<number, number>>;
  hiddenCols: Accessor<number[]>;
  hiddenRows: Accessor<number[]>;
  selectionAnchor: Accessor<{ row: number; col: number }>;
  activeCell: Accessor<{ row: number; col: number }>;
  chartType: Accessor<"bar" | "line" | "pie" | "area" | "scatter" | "doughnut" | null>;
  chartTitle: Accessor<string>;
  chartRange: Accessor<{ startRow: number; endRow: number; startCol: number; endCol: number }>;
  filterQuery: Accessor<string>;
  merges: Accessor<import("./sheetTypes").MergeRange[]>;
  autoFilterEnabled: Accessor<boolean>;
  filterRange: Accessor<{ startRow: number; endRow: number; startCol: number; endCol: number } | null>;
  columnFilters: Accessor<Record<number, string[]>>;
  namedRanges: Accessor<Array<{ name: string; rangeStr: string; sheet: string | null }>>;
  conditionalFormatting?: Accessor<ConditionalFormattingRule[]>;
  pivotTables?: Accessor<PivotTableConfig[]>;
  scenarios?: Accessor<ScenarioConfig[]>;
  slicers?: Accessor<SlicerConfig[]>;
  comments?: Accessor<import("./sheetTypes").CellComment[]>;
}

export function buildWorkbookFromCells(ctx: BuildWorkbookContext, cellMap: Record<string, GridCell>) {
  const existing = ctx.initialContent?.sheets?.length
    ? structuredClone(ctx.initialContent)
    : { sheets: [{ id: "sheet-1", name: "Sheet1", cells: {}, colWidths: {}, rowHeights: {}, freezeRows: 0, freezeCols: 0, conditionalFormatting: [], spills: [] }], activeSheetIndex: 0 };
  const currentMeta = ctx.sheets();
  if (existing.sheets) {
    existing.sheets = currentMeta.map((s, idx) => {
      const cached = ctx.sheetDataCache()[s.id];
      const found = cached || existing.sheets?.find((es: any) => es.id === s.id) || existing.sheets?.[idx];
      return {
        id: s.id,
        name: s.name,
        tabColor: s.color || found?.tabColor || undefined,
        cells: found?.cells || {},
        colWidths: found?.colWidths || {},
        rowHeights: found?.rowHeights || {},
        hiddenCols: idx === ctx.activeSheetIndex() ? ctx.hiddenCols() : found?.hiddenCols || [],
        hiddenRows: idx === ctx.activeSheetIndex() ? ctx.hiddenRows() : found?.hiddenRows || [],
        freezeRows: found?.freezeRows || 0,
        freezeCols: found?.freezeCols || 0,
        charts: found?.charts || [],
        filterQuery: found?.filterQuery || null,
        merges: found?.merges || [],
        autoFilter: found?.autoFilter || null,
        conditionalFormatting: found?.conditionalFormatting || [],
        pivotTables: found?.pivotTables || [],
        tables: found?.tables || [],
        scenarios: found?.scenarios || [],
        slicers: found?.slicers || [],
        spills: found?.spills || [],
        comments: found?.comments || [],
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
  // Single-chart editing context: the active chart replaces slot 0; when no
  // chart is being edited, every persisted chart survives untouched.
  const preservedCharts = ctx.chartType()
    ? (sheet.charts || []).slice(1)
    : (sheet.charts || []);
  const activeChart = ctx.chartType()
    ? [{
        chartType: ctx.chartType(),
        title: ctx.chartTitle(),
        startRow: ctx.chartRange().startRow,
        endRow: ctx.chartRange().endRow,
        startCol: ctx.chartRange().startCol,
        endCol: ctx.chartRange().endCol,
      }]
    : [];
  sheet.charts = [...activeChart, ...preservedCharts];
  sheet.filterQuery = ctx.filterQuery() || null;
  sheet.merges = ctx.merges();
  sheet.conditionalFormatting = ctx.conditionalFormatting?.() ?? [];
  sheet.pivotTables = ctx.pivotTables?.() ?? [];
  sheet.scenarios = ctx.scenarios?.() ?? [];
  sheet.slicers = ctx.slicers?.() ?? [];
  sheet.comments = ctx.comments?.() ?? [];
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
  // Dynamic-array spill values are render-only. Persist the origin formula and
  // the bounded spill metadata, never synthetic neighbor cells as literals.
  const materializedCells = Object.fromEntries(
    Object.entries(cellMap).filter(([, value]) => !value.spill),
  );
  for (const pivot of sheet.pivotTables) {
    const rowCount = pivot.outputRowCount || 0;
    const colCount = Math.max(2, Number(pivot.outputColCount) || 2);
    for (let row = pivot.outputStartRow; row < pivot.outputStartRow + rowCount; row += 1) {
      for (let col = 0; col < colCount; col += 1) {
        delete materializedCells[`${row}:${pivot.outputStartCol + col}`];
      }
    }
    const result = buildPivotTable(
      materializedCells,
      pivot,
      (row) => rowMatchesSlicers(materializedCells, row, sheet.slicers),
    );
    if (!result.warning) {
      pivot.outputRowCount = result.rowCount;
      Object.assign(materializedCells, result.cells);
    }
  }
  sheet.cells = Object.fromEntries(Object.entries(materializedCells).map(([key, value]) => [key, {
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
          formatCode: value.style.formatCode,
          decimals: value.style.decimals,
          hyperlink: value.style.hyperlink,
          validation: value.style.validation,
          wrap: value.style.wrap,
          vAlign: value.style.vAlign,
          fontFamily: value.style.fontFamily,
          fontSize: value.style.fontSize,
          image: value.style.image,
          borders: value.style.borders,
        }
      : null,
  }]));
  existing.activeSheetIndex = ctx.activeSheetIndex();
  existing.namedRanges = ctx.namedRanges();
  const cache: Record<string, any> = { ...ctx.sheetDataCache() };
  existing.sheets?.forEach((s: any) => {
    if (s?.id) cache[s.id] = s;
  });
  ctx.setSheetDataCache(cache);
  return existing;
}
