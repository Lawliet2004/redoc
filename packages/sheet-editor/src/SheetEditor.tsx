import { createEffect, createSignal, onCleanup, onMount, For, Show } from "solid-js";
import {
  IconPlus, IconBold, IconItalic, IconUnderline, IconTextColor, IconHighlight,
  IconAlignLeft, IconAlignCenter, IconAlignRight, IconAlignTop, IconAlignMiddle, IconAlignBottom,
  IconUndo, IconRedo, IconCut, IconCopy, IconPaste, IconNew, IconFolderOpen, IconSave, IconPdf,
  IconPrint, IconSortAsc, IconSortDesc, IconFilter, IconChart, IconImage, IconCurrency, IconPercent,
  IconMerge, IconWrap, IconSum, IconEquals, IconFreeze, IconProperties, IconStyles, IconGallery,
  IconNavigator, IconFunctions,
} from "@redoc/icons";
import {
  ToolbarRow, ToolbarButton, ToolbarSep, ToolbarSelect, ToolbarColor,
  FindBar, IconSidebar, type SidebarPanel,
  ContextMenu, type ContextMenuItem,
  EDITOR_COMMAND, type EditorCommandDetail, emitEditorCommand,
} from "@redoc/editor-common";
import { Dialog, showToast } from "@redoc/ui";
import { FormulaBar } from "./FormulaBar";
import { SheetToolbar } from "./SheetToolbar";
import { FORMULA_FUNCTION_NAMES } from "./formulaFunctions";
import { GridCanvas } from "./GridCanvas";
import { NumberFormatDialog, type NumberFormatValue } from "./NumberFormatDialog";
import { DataValidationDialog, type ListValidation } from "./DataValidationDialog";
import { measureTextWidth, clearTextMeasureCache } from "./textMeasureCache";
import { commands } from "@redoc/api-client";
import { parseTsv } from "@redoc/utils";
import { buildWorkbookFromCells, createSheetHistoryHandlers } from "./sheetModel";
import type { ConditionalFormattingRule, GridCell, MergeRange, PivotTableConfig, ScenarioConfig, SheetSnapshot, SlicerConfig, CellComment } from "./sheetTypes";
import { conditionalStyleForCell } from "./conditionalFormatting";
import { buildPivotTable, buildMultiFieldPivotTable } from "./pivotTables";
import { applyScenario as applyScenarioOverrides, captureScenario as captureScenarioRange } from "./scenarios";
import { createSlicer, rowMatchesSlicers, slicerValues, toggleSlicerValue } from "./slicers";
import {
  commentsForCell,
  makeCellComment,
  openCommentCount,
  removeCellComment,
  setCellCommentResolved,
  upsertCellComment,
} from "./cellComments";
import { normalizeSheetHyperlink, parseInternalSheetLocation } from "./hyperlinks";
import { setRichClipboard, takeRichClipboard, shiftFormulaReferences, type RichClipboard } from "./richClipboard";
import { SortDialog } from "./dialogs/SortDialog";
import { PrintDialog } from "./dialogs/PrintDialog";

interface SheetEditorProps {
  initialContent?: any;
  onChange?: (jsonContent: any) => void;
  onCellInfoChange?: (info: string) => void;
  onExportCsv?: () => void;
  onImportCsv?: () => Promise<any | undefined>;
  onRequestNew?: () => void;
  onRequestOpen?: () => void;
  onRequestSave?: () => void;
  onRequestExportPdf?: () => void;
  zoomLevel?: number;
  /** Enables the platform spellchecker on the cell/formula inputs. */
  spellcheckEnabled?: boolean;
  /** Display name used to attribute cell comments. */
  authorName?: string;
}

export function SheetEditor(props: SheetEditorProps) {
  let canvasRef!: HTMLCanvasElement;
  let containerRef!: HTMLDivElement;
  let formulaInputRef!: HTMLInputElement;

  const [activeCell, setActiveCell] = createSignal<{ row: number; col: number }>({ row: 1, col: 1 });
  const [selectionAnchor, setSelectionAnchor] = createSignal<{ row: number; col: number }>({ row: 1, col: 1 });
  const [formulaValue, setFormulaValue] = createSignal("");
  const [editing, setEditing] = createSignal(false);
  const [editInput, setEditInput] = createSignal("");
  const [sheets, setSheets] = createSignal<Array<{ id: string; name: string }>>([
    { id: "sheet-1", name: "Sheet1" },
  ]);
  const [activeSheetIndex, setActiveSheetIndex] = createSignal(0);
  const [scrollTop, setScrollTop] = createSignal(0);
  const [scrollLeft, setScrollLeft] = createSignal(0);
  const [freezeRows, setFreezeRows] = createSignal(0);
  const [freezeCols, setFreezeCols] = createSignal(0);
  const [chartType, setChartType] = createSignal<"bar" | "line" | "pie" | "area" | "scatter" | "doughnut" | null>(null);
  const [filterQuery, setFilterQuery] = createSignal("");
  const [findQuery, setFindQuery] = createSignal("");
  const [replaceWith, setReplaceWith] = createSignal("");
  const [findBarOpen, setFindBarOpen] = createSignal(false);
  const [findReplaceMode, setFindReplaceMode] = createSignal(false);
  const [namedRanges, setNamedRanges] = createSignal<Array<{ name: string; rangeStr: string; sheet: string | null }>>([]);
  const [matchCase, setMatchCase] = createSignal(false);
  const [filterOpen, setFilterOpen] = createSignal(false);
  const [fontFamily, setFontFamily] = createSignal("Liberation Sans");
  const [fontSize, setFontSize] = createSignal("10");
  const [historyPast, setHistoryPast] = createSignal<SheetSnapshot[]>([]);
  const [historyFuture, setHistoryFuture] = createSignal<SheetSnapshot[]>([]);
  const [merges, setMerges] = createSignal<MergeRange[]>([]);
  const [autoFilterEnabled, setAutoFilterEnabled] = createSignal(false);
  const [filterRange, setFilterRange] = createSignal<{
    startRow: number;
    endRow: number;
    startCol: number;
    endCol: number;
  } | null>(null);
  const [columnFilters, setColumnFilters] = createSignal<Record<number, string[]>>({});
  const [conditionalFormatting, setConditionalFormatting] = createSignal<ConditionalFormattingRule[]>([]);
  const [conditionalFormatType, setConditionalFormatType] = createSignal<ConditionalFormattingRule["type"]>("greaterThan");
  const [conditionalFormatValue, setConditionalFormatValue] = createSignal("0");
  const [conditionalFormatColor, setConditionalFormatColor] = createSignal("#dcfce7");
  const [pivotTables, setPivotTables] = createSignal<PivotTableConfig[]>([]);
  const [pivotRowField, setPivotRowField] = createSignal(1);
  const [pivotValueField, setPivotValueField] = createSignal(2);
  const [pivotAggregation, setPivotAggregation] = createSignal<PivotTableConfig["aggregation"]>("sum");
  const [pivotColumnField, setPivotColumnField] = createSignal<number | "">("");
  const [scenarios, setScenarios] = createSignal<ScenarioConfig[]>([]);
  const [scenarioName, setScenarioName] = createSignal("");
  const [slicers, setSlicers] = createSignal<SlicerConfig[]>([]);
  const [cellComments, setCellComments] = createSignal<CellComment[]>([]);
  const [commentDraft, setCommentDraft] = createSignal("");
  const [slicerColumn, setSlicerColumn] = createSignal(1);
  const [slicerTitle, setSlicerTitle] = createSignal("");
  const [filterDropdownCol, setFilterDropdownCol] = createSignal<number | null>(null);
  const [sortDialogOpen, setSortDialogOpen] = createSignal(false);
  const [sortKeys, setSortKeys] = createSignal<Array<{ col: number; ascending: boolean }>>([
    { col: 1, ascending: true },
  ]);
  const [chartTitle, setChartTitle] = createSignal("Chart");
  const [chartRange, setChartRange] = createSignal({ startRow: 1, endRow: 3, startCol: 1, endCol: 2 });
  const [printDialogOpen, setPrintDialogOpen] = createSignal(false);
  const [printMode, setPrintMode] = createSignal<"sheet" | "selection">("sheet");
  const [printFitWidth, setPrintFitWidth] = createSignal(true);
  const [fillDragStart, setFillDragStart] = createSignal<{ row: number; col: number } | null>(null);
  const [suppressNextClick, setSuppressNextClick] = createSignal(false);
  const [dimensionDrag, setDimensionDrag] = createSignal<{
    axis: "column" | "row";
    index: number;
    pointer: number;
    size: number;
  } | null>(null);
  const [contextMenu, setContextMenu] = createSignal<{ x: number; y: number; items: ContextMenuItem[] } | null>(null);
  const [numberFormatOpen, setNumberFormatOpen] = createSignal(false);
  const [validationOpen, setValidationOpen] = createSignal(false);
  const [hyperlinkDraft, setHyperlinkDraft] = createSignal("");

  const cellImageCache = new Map<string, HTMLImageElement | null>();

  let gridDirtyFull = true;
  let gridDirtyRect: { x: number; y: number; w: number; h: number } | null = null;

  const markGridDirtyFull = () => {
    gridDirtyFull = true;
    gridDirtyRect = null;
  };

  const markGridDirtyRect = (rect: { x: number; y: number; w: number; h: number }) => {
    if (gridDirtyFull) return;
    if (!gridDirtyRect) {
      gridDirtyRect = { ...rect };
      return;
    }
    const x1 = Math.min(gridDirtyRect.x, rect.x);
    const y1 = Math.min(gridDirtyRect.y, rect.y);
    const x2 = Math.max(gridDirtyRect.x + gridDirtyRect.w, rect.x + rect.w);
    const y2 = Math.max(gridDirtyRect.y + gridDirtyRect.h, rect.y + rect.h);
    gridDirtyRect = { x: x1, y: y1, w: x2 - x1, h: y2 - y1 };
  };

  const markSelectionDirty = () => {
    const bounds = selectedBounds();
    const x = headerColWidth - scrollLeft() + offsetForColumn(bounds.startCol);
    const y = headerRowHeight - scrollTop() + offsetForRow(bounds.startRow);
    const w = offsetForColumn(bounds.endCol + 1) - offsetForColumn(bounds.startCol);
    const h = offsetForRow(bounds.endRow + 1) - offsetForRow(bounds.startRow);
    markGridDirtyRect({
      x: Math.max(0, x),
      y: Math.max(0, y),
      w: Math.max(1, w),
      h: Math.max(1, h),
    });
  };

  // Grid constants
  const [columnWidth, setColumnWidth] = createSignal(100);
  const [rowHeight, setRowHeight] = createSignal(26);
  const [columnWidths, setColumnWidths] = createSignal<Record<number, number>>({});
  const [rowHeights, setRowHeights] = createSignal<Record<number, number>>({});
  const headerColWidth = 40;
  const headerRowHeight = 26;

  const widthAt = (col: number) => columnWidths()[col] || columnWidth();
  const heightAt = (row: number) => rowHeights()[row] || rowHeight();
  const offsetForColumn = (col: number) => {
    let offset = 0;
    for (let index = 1; index < col; index += 1) offset += widthAt(index);
    return offset;
  };
  const offsetForRow = (row: number) => {
    let offset = 0;
    for (let index = 1; index < row; index += 1) offset += heightAt(index);
    return offset;
  };
  const columnAtOffset = (offset: number) => {
    let remaining = Math.max(0, offset);
    let col = 1;
    while (remaining >= widthAt(col) && col < 1000) {
      remaining -= widthAt(col);
      col += 1;
    }
    return { col, remainder: remaining };
  };
  const rowAtOffset = (offset: number) => {
    let remaining = Math.max(0, offset);
    let row = 1;
    while (remaining >= heightAt(row) && row < 100000) {
      remaining -= heightAt(row);
      row += 1;
    }
    return { row, remainder: remaining };
  };

  const [sheetDataCache, setSheetDataCache] = createSignal<Record<number, any>>({});

  // Cells data store: "row:col" -> { raw, display }
  const [cellsData, setCellsData] = createSignal<Record<string, GridCell>>({
    "1:1": { raw: "Item", display: "Item" },
    "1:2": { raw: "Price", display: "Price" },
    "1:3": { raw: "Qty", display: "Qty" },
    "1:4": { raw: "Total", display: "Total" },
    "2:1": { raw: "Widget A", display: "Widget A" },
    "2:2": { raw: "15", display: "15" },
    "2:3": { raw: "4", display: "4" },
    "2:4": { raw: "=15*4", display: "60" },
    "3:1": { raw: "Widget B", display: "Widget B" },
    "3:2": { raw: "25", display: "25" },
    "3:3": { raw: "2", display: "2" },
    "3:4": { raw: "=25*2", display: "50" },
  });

  const MAX_RENDER_SPILL_CELLS = 100_000;

  const gridCellsFromSheet = (sheet: any): Record<string, GridCell> => {
    const loaded: Record<string, GridCell> = {};
    for (const [key, cell] of Object.entries(sheet?.cells || {})) {
      const value = cell as any;
      loaded[key] = {
        raw: value.formula ?? value.rawValue ?? value.raw ?? "",
        display: value.displayValue ?? value.display ?? value.rawValue ?? "",
        style: value.style ?? undefined,
      };
    }
    // Keep dynamic-array results visible without turning spill neighbors into
    // persisted literals. Explicit cells always win, matching spreadsheet
    // providers that expose a bounded materialized result to the renderer.
    for (const spill of Array.isArray(sheet?.spills) ? sheet.spills : []) {
      const originRow = Number(spill?.originRow);
      const originCol = Number(spill?.originCol);
      const rows = Number(spill?.rows);
      const cols = Number(spill?.cols);
      if (!Number.isSafeInteger(originRow) || !Number.isSafeInteger(originCol)
        || originRow < 1 || originCol < 1 || !Number.isSafeInteger(rows)
        || !Number.isSafeInteger(cols) || rows < 1 || cols < 1) continue;
      const values = Array.isArray(spill?.values) ? spill.values : [];
      const count = Math.min(rows * cols, values.length, MAX_RENDER_SPILL_CELLS);
      for (let index = 0; index < count; index += 1) {
        const row = originRow + Math.floor(index / cols);
        const col = originCol + (index % cols);
        const key = `${row}:${col}`;
        if (loaded[key]) continue;
        loaded[key] = {
          raw: "",
          display: typeof values[index] === "string" ? values[index] : String(values[index] ?? ""),
          spill: true,
          style: { fontColor: "#475569" },
        };
      }
    }
    return loaded;
  };

  /** Returns bounded cached cell maps so validation lists can read another sheet. */
  const validationCellsBySheet = () => {
    const result: Record<string, Record<string, GridCell>> = {};
    for (const [index, sheet] of sheets().entries()) {
      const source = index === activeSheetIndex()
        ? null
        : sheetDataCache()[index] || props.initialContent?.sheets?.[index];
      result[sheet.name] = source ? gridCellsFromSheet(source) : (index === activeSheetIndex() ? cellsData() : {});
    }
    return result;
  };

  const loadSheet = (sheet: any) => {
    if (!sheet) return;
    setCellsData(gridCellsFromSheet(sheet));
    setFreezeRows(sheet.freezeRows || 0);
    setFreezeCols(sheet.freezeCols || 0);
    setColumnWidth(sheet.colWidths?.[0] || 100);
    setRowHeight(sheet.rowHeights?.[0] || 26);
    setColumnWidths(sheet.colWidths || {});
    setRowHeights(sheet.rowHeights || {});
    setChartType(sheet.charts?.[0]?.chartType || null);
    if (sheet.charts?.[0]) {
      setChartTitle(sheet.charts[0].title || "Chart");
      setChartRange({
        startRow: sheet.charts[0].startRow || 1,
        endRow: sheet.charts[0].endRow || 3,
        startCol: sheet.charts[0].startCol || 1,
        endCol: sheet.charts[0].endCol || 2,
      });
    }
    setFilterQuery(sheet.filterQuery || "");
    setMerges(sheet.merges || []);
    const loadedFilter = sheet.autoFilter;
    setAutoFilterEnabled(!!loadedFilter?.enabled);
    setFilterRange(loadedFilter?.enabled ? {
      startRow: loadedFilter.startRow || 1,
      endRow: loadedFilter.endRow || 100,
      startCol: loadedFilter.startCol || 1,
      endCol: loadedFilter.endCol || 10,
    } : null);
    const loadedColumnFilters: Record<number, string[]> = {};
    for (const [column, values] of Object.entries(loadedFilter?.columnFilters || {})) {
      loadedColumnFilters[Number(column)] = Array.isArray(values) ? values as string[] : [];
    }
    setColumnFilters(loadedColumnFilters);
    setConditionalFormatting(sheet.conditionalFormatting || []);
    setPivotTables(sheet.pivotTables || []);
    setScenarios(sheet.scenarios || []);
    setSlicers(sheet.slicers || []);
    setCellComments(sheet.comments || []);
  };

  const makeWorkbook = (cellMap: Record<string, GridCell>) =>
    buildWorkbookFromCells(
      {
        initialContent: props.initialContent,
        sheets,
        sheetDataCache,
        setSheetDataCache,
        activeSheetIndex,
        freezeRows,
        freezeCols,
        columnWidth,
        rowHeight,
        columnWidths,
        rowHeights,
        selectionAnchor,
        activeCell,
        chartType,
        chartTitle,
        chartRange,
        filterQuery,
        merges,
        autoFilterEnabled,
        filterRange,
        columnFilters,
        namedRanges,
        conditionalFormatting,
        pivotTables,
        scenarios,
        slicers,
        comments: cellComments,
      },
      cellMap,
    );

  const refreshPivotCells = (
    sourceCells: Record<string, GridCell>,
    activeSlicers: SlicerConfig[],
  ) => {
    const next = { ...sourceCells };
    const updated = pivotTables().map((pivot) => {
      clearPivotOutput(next, pivot);
      if (pivot.columnField != null) {
        const result = buildMultiFieldPivotTable(
          next,
          {
            sourceRange: pivot.sourceRange,
            rowField: pivot.rowField,
            columnField: pivot.columnField,
            valueField: pivot.valueField,
            aggregation: pivot.aggregation,
            outputStartRow: pivot.outputStartRow,
            outputStartCol: pivot.outputStartCol,
          },
          (row) => rowMatchesSlicers(next, row, activeSlicers),
        );
        if (result.warning) return pivot;
        Object.assign(next, result.cells);
        return { ...pivot, outputRowCount: result.rowCount, outputColCount: result.colCount };
      }
      const result = buildPivotTable(
        next,
        pivot,
        (row) => rowMatchesSlicers(next, row, activeSlicers),
      );
      if (result.warning) return pivot;
      Object.assign(next, result.cells);
      return { ...pivot, outputRowCount: result.rowCount };
    });
    setPivotTables(updated);
    return next;
  };

  /** Remove a pivot's written output cells from a cell map, in place. */
  const clearPivotOutput = (cells: Record<string, GridCell>, pivot: PivotTableConfig) => {
    const rows = pivot.outputRowCount || 0;
    const cols = pivot.outputColCount || (pivot.columnField != null ? 0 : 2) || 2;
    for (let row = pivot.outputStartRow; row < pivot.outputStartRow + rows; row += 1) {
      for (let col = pivot.outputStartCol; col < pivot.outputStartCol + cols; col += 1) {
        delete cells[`${row}:${col}`];
      }
    }
  };

  onMount(() => {
    if (props.initialContent?.sheets?.length) {
      setSheets(props.initialContent.sheets.map((sheet: any) => ({ id: sheet.id, name: sheet.name })));
      const index = props.initialContent.activeSheetIndex || 0;
      setActiveSheetIndex(index);
      setFreezeRows(props.initialContent.sheets[index].freezeRows || 0);
      setFreezeCols(props.initialContent.sheets[index].freezeCols || 0);
      setColumnWidth(props.initialContent.sheets[index].colWidths?.[0] || 100);
      setRowHeight(props.initialContent.sheets[index].rowHeights?.[0] || 26);
      setColumnWidths(props.initialContent.sheets[index].colWidths || {});
      setRowHeights(props.initialContent.sheets[index].rowHeights || {});
      setMerges(props.initialContent.sheets[index].merges || []);
      const loadedFilter = props.initialContent.sheets[index].autoFilter;
      setAutoFilterEnabled(!!loadedFilter?.enabled);
      if (loadedFilter?.enabled) {
        setFilterRange({
          startRow: loadedFilter.startRow || 1,
          endRow: loadedFilter.endRow || 100,
          startCol: loadedFilter.startCol || 1,
          endCol: loadedFilter.endCol || 10,
        });
      }
      if (loadedFilter?.columnFilters) {
        const raw = loadedFilter.columnFilters;
        const mapped: Record<number, string[]> = {};
        for (const [k, v] of Object.entries(raw)) mapped[Number(k)] = v as string[];
        setColumnFilters(mapped);
      }
      if (props.initialContent.sheets[index].charts?.[0]) {
        const chart = props.initialContent.sheets[index].charts[0];
        setChartType(chart.chartType || "bar");
        setChartTitle(chart.title || "Chart");
        setChartRange({
          startRow: chart.startRow || 1,
          endRow: chart.endRow || 3,
          startCol: chart.startCol || 1,
          endCol: chart.endCol || 2,
        });
      }
      loadSheet(props.initialContent.sheets[index]);
      setNamedRanges(props.initialContent.namedRanges || []);
    }
  });

  const findMerge = (r: number, c: number) => {
    for (const m of merges()) {
      if (r >= m.startRow && r <= m.endRow && c >= m.startCol && c <= m.endCol) {
        return m;
      }
    }
    return null;
  };

  const getColName = (c: number) => {
    let s = "";
    let col = c;
    while (col > 0) {
      const rem = (col - 1) % 26;
      s = String.fromCharCode(65 + rem) + s;
      col = Math.floor((col - 1) / 26);
    }
    return s;
  };

  const formatDisplay = (value: string, format?: NonNullable<GridCell["style"]>["format"], decimals?: number) => {
    if (!format || format === "general") return value;
    if (format === "text") return value;
    if (value.trim() === "") return value;
    const number = Number(value);
    if (!Number.isFinite(number)) return value;
    const d = typeof decimals === "number" ? decimals : 2;
    if (format === "currency") return `$${number.toFixed(d)}`;
    if (format === "percent") return `${(number * 100).toFixed(d)}%`;
    if (format === "date") {
      const parsed = Date.parse(value);
      if (!Number.isNaN(parsed)) {
        return new Date(parsed).toLocaleDateString(undefined, { year: "numeric", month: "short", day: "numeric" });
      }
      const serial = Number(value);
      if (Number.isFinite(serial) && serial > 0) {
        const epoch = new Date(Date.UTC(1899, 11, 30));
        epoch.setUTCDate(epoch.getUTCDate() + Math.floor(serial));
        return epoch.toLocaleDateString(undefined, { year: "numeric", month: "short", day: "numeric" });
      }
      return value;
    }
    return number.toLocaleString(undefined, { minimumFractionDigits: d, maximumFractionDigits: d });
  };

  const looksLikeUrl = (text: string) => /^https?:\/\//i.test(text) || /^www\./i.test(text);
  const cellHyperlink = (cell: GridCell | undefined): string | null => {
    if (!cell) return null;
    if (cell.style?.hyperlink) {
      return normalizeSheetHyperlink(cell.style.hyperlink);
    }
    const raw = (cell.raw || cell.display || "").trim();
    if (!raw || raw.startsWith("=")) return null;
    if (looksLikeUrl(raw)) return /^https?:\/\//i.test(raw) ? raw : `https://${raw}`;
    return null;
  };

  const openSheetHyperlink = (href: string): boolean => {
    const location = parseInternalSheetLocation(href);
    if (!location) {
      if (/^internal:/i.test(href)) showToast("This internal cell link is malformed or out of bounds.", "warning");
      return /^internal:/i.test(href);
    }
    const targetIndex = location.sheetName
      ? sheets().findIndex((sheet) => sheet.name.toLowerCase() === location.sheetName!.toLowerCase())
      : activeSheetIndex();
    if (targetIndex < 0) {
      showToast(`Sheet “${location.sheetName}” was not found.`, "warning");
      return true;
    }
    const workbook = makeWorkbook(cellsData());
    props.onChange?.(workbook);
    setActiveSheetIndex(targetIndex);
    const source = workbook.sheets?.[targetIndex];
    if (source) loadSheet(source);
    setActiveCell({ row: location.row, col: location.col });
    setSelectionAnchor({ row: location.row, col: location.col });
    setFormulaValue(source?.cells?.[`${location.row}:${location.col}`]?.formula || source?.cells?.[`${location.row}:${location.col}`]?.rawValue || "");
    emitCellInfo(location.row, location.col);
    markSelectionDirty();
    drawGrid();
    return true;
  };

  const chartData = () => {
    const range = chartRange();
    const startRow = range.startRow;
    const endRow = range.endRow;
    const startCol = range.startCol;
    const endCol = range.endCol;
    const valueCol = startCol < endCol ? startCol + 1 : startCol;
    return Array.from({ length: Math.max(0, endRow - startRow + 1) }, (_, index) => {
      const row = startRow + index;
      const label = cellsData()[`${row}:${startCol}`]?.display || `${row}`;
      const value = Number(cellsData()[`${row}:${valueCol}`]?.display || 0);
      return { label, value: Number.isFinite(value) ? value : 0 };
    }).slice(0, 20);
  };

  const chartMax = () => Math.max(1, ...chartData().map((item) => Math.abs(item.value)));

  const pieSlices = () => {
    const data = chartData();
    const total = data.reduce((sum, item) => sum + Math.max(0, Math.abs(item.value)), 0);
    if (total <= 0) return [];

    const colors = ["#3b82f6", "#ef4444", "#10b981", "#f59e0b", "#8b5cf6", "#ec4899", "#14b8a6", "#f97316", "#6366f1", "#84cc16"];
    const cx = 190;
    const cy = 115;
    const r = 80;

    let currentAngle = -Math.PI / 2;
    return data.map((item, index) => {
      const val = Math.max(0, Math.abs(item.value));
      const portion = val / total;
      const angle = portion * 2 * Math.PI;
      const startAngle = currentAngle;
      const endAngle = currentAngle + angle;
      currentAngle = endAngle;

      const color = colors[index % colors.length];

      if (portion >= 0.9999) {
        return { isFullCircle: true, pathData: "", color, label: item.label, value: item.value };
      }

      const x1 = cx + r * Math.cos(startAngle);
      const y1 = cy + r * Math.sin(startAngle);
      const x2 = cx + r * Math.cos(endAngle);
      const y2 = cy + r * Math.sin(endAngle);
      const largeArcFlag = angle > Math.PI ? 1 : 0;

      const pathData = `M ${cx} ${cy} L ${x1.toFixed(2)} ${y1.toFixed(2)} A ${r} ${r} 0 ${largeArcFlag} 1 ${x2.toFixed(2)} ${y2.toFixed(2)} Z`;

      return {
        isFullCircle: false,
        pathData,
        color,
        label: item.label,
        value: item.value,
      };
    });
  };

  const filterBounds = () => {
    if (!autoFilterEnabled()) return null;
    return filterRange() || selectedBounds();
  };

  const parseColLetter = (text: string) => {
    const cleaned = text.trim().toUpperCase();
    if (!/^[A-Z]+$/.test(cleaned)) return null;
    let col = 0;
    for (const ch of cleaned) col = col * 26 + (ch.charCodeAt(0) - 64);
    return col > 0 ? col : null;
  };

  const usedRegion = () => {
    let maxRow = 1;
    let maxCol = 1;
    for (const key of Object.keys(cellsData())) {
      const [r, c] = key.split(":").map(Number);
      if (r > maxRow) maxRow = r;
      if (c > maxCol) maxCol = c;
    }
    return { startRow: 1, endRow: Math.max(2, maxRow), startCol: 1, endCol: Math.max(1, maxCol) };
  };

  const uniqueColumnValues = (col: number) => {
    const bounds = filterBounds();
    if (!bounds) return [] as string[];
    const values = new Set<string>();
    for (let row = bounds.startRow + 1; row <= bounds.endRow; row += 1) {
      values.add(cellsData()[`${row}:${col}`]?.display || "");
    }
    return Array.from(values).sort((a, b) => a.localeCompare(b)).slice(0, 200);
  };

  const rowMatchesFilter = (row: number) => {
    if (!rowMatchesSlicers(cellsData(), row, slicers())) return false;
    const bounds = filterBounds();
    if (!autoFilterEnabled() || !bounds) {
      const query = filterQuery().trim().toLowerCase();
      if (!query) return true;
      const value = cellsData()[`${row}:${activeCell().col}`]?.display || "";
      return value.toLowerCase().includes(query);
    }
    if (row <= bounds.startRow || row > bounds.endRow) return true;
    const filters = columnFilters();
    for (const [colStr, allowed] of Object.entries(filters)) {
      const col = Number(colStr);
      if (!allowed || allowed.length === 0) continue;
      const value = cellsData()[`${row}:${col}`]?.display || "";
      if (!allowed.includes(value)) return false;
    }
    const query = filterQuery().trim().toLowerCase();
    if (query) {
      const value = cellsData()[`${row}:${activeCell().col}`]?.display || "";
      if (!value.toLowerCase().includes(query)) return false;
    }
    return true;
  };

  const hasHiddenFilteredRows = () => {
    if (!autoFilterEnabled()) return false;
    const bounds = filterBounds();
    if (!bounds) return false;
    for (let row = bounds.startRow + 1; row <= bounds.endRow; row += 1) {
      if (!rowMatchesFilter(row)) return true;
    }
    return false;
  };

  const visualRowAtOffset = (offset: number) => {
    let remaining = Math.max(0, offset);
    let row = 1;
    while (row < 100000) {
      if (!rowMatchesFilter(row)) {
        row += 1;
        continue;
      }
      const h = heightAt(row);
      if (remaining < h) return { row, remainder: remaining };
      remaining -= h;
      row += 1;
    }
    return { row, remainder: 0 };
  };

  const updateChart = (type: "bar" | "line" | "pie" | "area" | "scatter" | "doughnut" | null) => {
    pushHistory();
    setChartType(type);
    if (type) {
      const bounds = selectedBounds();
      setChartRange({
        startRow: bounds.startRow,
        endRow: bounds.endRow,
        startCol: bounds.startCol,
        endCol: bounds.endCol,
      });
    }
    queueMicrotask(() => props.onChange?.(makeWorkbook(cellsData())));
  };

  const cellFont = (style: GridCell["style"] | undefined, row?: number) => {
    const size = style?.fontSize || Number(fontSize()) || 12;
    const family = style?.fontFamily || fontFamily() || "Inter, sans-serif";
    const weight = style?.bold || row === 1 ? "bold" : "normal";
    const italic = style?.italic ? "italic" : "normal";
    return `${italic === "normal" ? "" : italic + " "}${weight === "normal" ? "" : weight + " "}${size}px ${family}`;
  };

  const autoHeightForWrappedRows = (cellMap: Record<string, GridCell>, rows: number[]) => {
    const ctx = canvasRef?.getContext("2d");
    if (!ctx) return;
    const nextHeights = { ...rowHeights() };
    const lineHeight = 14;
    const padding = 10;
    for (const r of rows) {
      let maxLines = 1;
      for (let c = 1; c <= 50; c += 1) {
        const cell = cellMap[`${r}:${c}`];
        if (!cell?.style?.wrap) continue;
        const width = Math.max(24, widthAt(c) - 12);
        const text = cell.display || "";
        const fontStr = cellFont(cell.style, r);
        const words = text.split(/\s+/);
        let currentLineWidth = 0;
        let lines = 1;
        for (const word of words) {
          const w = measureTextWidth(ctx, fontStr, word + " ");
          if (currentLineWidth + w > width && currentLineWidth > 0) {
            lines++;
            currentLineWidth = w;
          } else {
            currentLineWidth += w;
          }
        }
        if (lines > maxLines) maxLines = lines;
      }
      const height = Math.max(rowHeight(), maxLines * lineHeight + padding);
      if (height !== (nextHeights[r] || rowHeight())) nextHeights[r] = height;
    }
    setRowHeights(nextHeights);
  };

  const drawGrid = () => {
    if (!canvasRef) return;
    const ctx = canvasRef.getContext("2d");
    if (!ctx) return;

    const drawTextWithUnderline = (
      text: string,
      x: number,
      y: number,
      style: GridCell["style"] | undefined,
      align: string,
      link?: string | null,
    ) => {
      ctx.fillText(text, x, y);
      if ((style?.underline || link) && text) {
        const font = ctx.font;
        const w = measureTextWidth(ctx, font, text);
        let startX = x;
        if (align === "center") startX = x - w / 2;
        else if (align === "right") startX = x - w;
        ctx.beginPath();
        ctx.moveTo(startX, y + 2);
        ctx.lineTo(startX + w, y + 2);
        ctx.strokeStyle = link ? "#2563eb" : style?.fontColor || "#0f172a";
        ctx.lineWidth = 1;
        ctx.stroke();
      }
    };

    const drawConditionalBar = (
      style: GridCell["style"] | undefined,
      x: number,
      y: number,
      width: number,
      height: number,
    ) => {
      const percent = style?.conditionalBarPercent;
      if (percent === undefined || !Number.isFinite(percent)) return;
      ctx.save();
      ctx.globalAlpha = 0.28;
      ctx.fillStyle = style?.conditionalBarColor || "#60a5fa";
      ctx.fillRect(x, y, width * Math.max(0, Math.min(1, percent)), height);
      ctx.restore();
    };

    const drawCellImage = (src: string | undefined, x: number, y: number, width: number, height: number) => {
      if (!src || width <= 8 || height <= 8) return;
      const cached = cellImageCache.get(src);
      if (cached === null) return;
      if (!cached) {
        const image = new Image();
        image.onload = () => {
          cellImageCache.set(src, image);
          markGridDirtyFull();
          drawGrid();
        };
        image.onerror = () => cellImageCache.set(src, null);
        cellImageCache.set(src, image);
        image.src = src;
        return;
      }
      if (!cached.complete || cached.naturalWidth <= 0 || cached.naturalHeight <= 0) return;
      const scale = Math.min((width - 6) / cached.naturalWidth, (height - 6) / cached.naturalHeight, 1);
      const imageWidth = Math.max(1, cached.naturalWidth * scale);
      const imageHeight = Math.max(1, cached.naturalHeight * scale);
      ctx.drawImage(cached, x + (width - imageWidth) / 2, y + (height - imageHeight) / 2, imageWidth, imageHeight);
    };

    const cssWidth = containerRef?.clientWidth || canvasRef.clientWidth;
    const cssHeight = containerRef?.clientHeight || canvasRef.clientHeight;
    if (gridDirtyFull) {
      ctx.clearRect(0, 0, cssWidth, cssHeight);
    } else if (gridDirtyRect) {
      ctx.clearRect(gridDirtyRect.x, gridDirtyRect.y, gridDirtyRect.w, gridDirtyRect.h);
    } else {
      ctx.clearRect(0, 0, cssWidth, cssHeight);
    }
    gridDirtyFull = false;
    gridDirtyRect = null;

    const width = cssWidth;
    const height = cssHeight;

    ctx.font = "12px Inter, sans-serif";

    // Draw header backgrounds
    ctx.fillStyle = "#f1f5f9";
    ctx.fillRect(0, 0, width, headerRowHeight);
    ctx.fillRect(0, 0, headerColWidth, height);

    // Grid lines & Headers
    ctx.strokeStyle = "#e2e8f0";
    ctx.lineWidth = 1;

    const columnStart = columnAtOffset(scrollLeft());
    const rowStart = rowAtOffset(scrollTop());
    const columns: Array<{ col: number; x: number; width: number }> = [];
    let columnX = headerColWidth - columnStart.remainder;
    const bufferWidth = 5 * columnWidth();
    while (columnX < width + bufferWidth && columns.length < 1000) {
      const col = columnStart.col + columns.length;
      const widthForColumn = widthAt(col);
      columns.push({ col, x: columnX, width: widthForColumn });
      columnX += widthForColumn;
    }
    const rows: Array<{ row: number; y: number; height: number }> = [];
    const compactRows = hasHiddenFilteredRows();
    const bufferHeight = 5 * rowHeight();
    if (compactRows) {
      let logicalRow = visualRowAtOffset(scrollTop()).row;
      let rowY = headerRowHeight - visualRowAtOffset(scrollTop()).remainder;
      while (rowY < height + bufferHeight && logicalRow < 100000) {
        while (logicalRow < 100000 && !rowMatchesFilter(logicalRow)) {
          logicalRow += 1;
        }
        if (logicalRow >= 100000) break;
        const heightForRow = heightAt(logicalRow);
        rows.push({ row: logicalRow, y: rowY, height: heightForRow });
        rowY += heightForRow;
        logicalRow += 1;
      }
    } else {
      let rowY = headerRowHeight - rowStart.remainder;
      while (rowY < height + bufferHeight && rows.length < 100000) {
        const row = rowStart.row + rows.length;
        const heightForRow = heightAt(row);
        rows.push({ row, y: rowY, height: heightForRow });
        rowY += heightForRow;
      }
    }

    // Columns
    for (const column of columns) {
      const c = column.col;
      const x = column.x;
      ctx.beginPath();
      ctx.moveTo(x, 0);
      ctx.lineTo(x, height);
      ctx.stroke();

      // Col Header text
      ctx.fillStyle = "#475569";
      ctx.textAlign = "center";
      ctx.textBaseline = "middle";
      const bounds = filterBounds();
      const showFilter =
        autoFilterEnabled() &&
        bounds &&
        c >= bounds.startCol &&
        c <= bounds.endCol;
      ctx.fillText(
        showFilter ? `${getColName(c)} ▾` : getColName(c),
        x + column.width / 2,
        headerRowHeight / 2,
      );
      if (showFilter && columnFilters()[c] && columnFilters()[c]!.length < uniqueColumnValues(c).length) {
        ctx.fillStyle = "#2563eb";
        ctx.beginPath();
        ctx.arc(x + column.width - 8, 8, 3, 0, Math.PI * 2);
        ctx.fill();
      }
    }

    // Rows
    for (const row of rows) {
      const r = row.row;
      const y = row.y;
      ctx.beginPath();
      ctx.moveTo(0, y);
      ctx.lineTo(width, y);
      ctx.stroke();

      // Row Header text
      ctx.fillStyle = "#475569";
      ctx.textAlign = "center";
      ctx.textBaseline = "middle";
      ctx.fillText(r.toString(), headerColWidth / 2, y + row.height / 2);
    }

    // Draw Cell Values
    const data = cellsData();
    ctx.textAlign = "left";
    ctx.textBaseline = "middle";

    const currentMerges = merges();

    const renderMergedCell = (m: MergeRange) => {
      const cellX = headerColWidth - scrollLeft() + offsetForColumn(m.startCol);
      const cellY = headerRowHeight - scrollTop() + offsetForRow(m.startRow);
      const cellWidth = offsetForColumn(m.endCol + 1) - offsetForColumn(m.startCol);
      const cellHeight = offsetForRow(m.endRow + 1) - offsetForRow(m.startRow);
      const key = `${m.startRow}:${m.startCol}`;
      const cell = data[key];
      const style = cell
        ? { ...cell.style, ...conditionalStyleForCell(cell, m.startRow, m.startCol, conditionalFormatting()) }
        : undefined;

      ctx.fillStyle = style?.bgColor || "#ffffff";
      ctx.fillRect(cellX, cellY, cellWidth, cellHeight);
      drawConditionalBar(style, cellX, cellY, cellWidth, cellHeight);
      drawCellImage(style?.image, cellX, cellY, cellWidth, cellHeight);

      ctx.strokeStyle = "#e2e8f0";
      ctx.lineWidth = 1;
      ctx.strokeRect(cellX, cellY, cellWidth, cellHeight);

      if (cell) {
        const link = cellHyperlink(cell);
        ctx.fillStyle = link ? "#2563eb" : style?.fontColor || "#0f172a";
        ctx.font = cellFont(style, m.startRow);
        ctx.textAlign = style?.align || "left";
        const textX = style?.align === "center" ? cellX + cellWidth / 2 : style?.align === "right" ? cellX + cellWidth - 6 : cellX + 6;
        const text = formatDisplay(cell.display, style?.format, style?.decimals);
        const vAlign = style?.vAlign || "middle";
        const textY = vAlign === "top" ? cellY + 8 : vAlign === "bottom" ? cellY + cellHeight - 8 : cellY + cellHeight / 2;
        ctx.textBaseline = vAlign === "top" ? "top" : vAlign === "bottom" ? "bottom" : "middle";
        const fontStr = cellFont(style, m.startRow);
        if (style?.wrap) {
          const maxWidth = cellWidth - 10;
          const words = text.split(/\s+/);
          let line = "";
          let lineY = vAlign === "middle" ? cellY + 12 : textY;
          for (const word of words) {
            const test = line ? `${line} ${word}` : word;
            if (measureTextWidth(ctx, fontStr, test) > maxWidth && line) {
              ctx.fillText(line, textX, lineY);
              line = word;
              lineY += 14;
              if (lineY > cellY + cellHeight - 4) break;
            } else {
              line = test;
            }
          }
          if (line && lineY <= cellY + cellHeight - 4) drawTextWithUnderline(line, textX, lineY, style, style?.align || "left", link);
        } else {
          drawTextWithUnderline(text, textX, textY, style, style?.align || "left", link);
        }
      }
    };

    const drawnMerges = new Set<string>();

    for (const row of rows) {
      const r = row.row;
      for (const column of columns) {
        const c = column.col;
        const merge = findMerge(r, c);
        if (merge) {
          const mergeKey = `${merge.startRow}:${merge.startCol}`;
          if (!drawnMerges.has(mergeKey)) {
            drawnMerges.add(mergeKey);
            renderMergedCell(merge);
          }
          continue;
        }

        const key = `${r}:${c}`;
        if (column.x > width || column.x + column.width < 0 || row.y > height || row.y + row.height < 0) continue;
        if (data[key]) {
          const x = column.x + 6;
          const y = row.y + row.height / 2;
          const cell = data[key];
          const style = { ...cell.style, ...conditionalStyleForCell(cell, r, c, conditionalFormatting()) };
          const link = cellHyperlink(cell);
          if (style?.bgColor) {
            ctx.fillStyle = style.bgColor;
            ctx.fillRect(column.x, row.y, column.width, row.height);
          }
          drawConditionalBar(style, column.x, row.y, column.width, row.height);
          drawCellImage(style?.image, column.x, row.y, column.width, row.height);
          ctx.fillStyle = link ? "#2563eb" : style?.fontColor || "#0f172a";
          const fontStr = cellFont(style, r);
          ctx.font = fontStr;
          ctx.textAlign = style?.align || "left";
          const textX = style?.align === "center" ? column.x + column.width / 2 : style?.align === "right" ? column.x + column.width - 6 : x;
          const text = formatDisplay(cell.display, style?.format, style?.decimals);
          const vAlign = style?.vAlign || "middle";
          const textY =
            vAlign === "top" ? row.y + 8 : vAlign === "bottom" ? row.y + row.height - 8 : y;
          ctx.textBaseline = vAlign === "top" ? "top" : vAlign === "bottom" ? "bottom" : "middle";
          if (style?.wrap) {
            const maxWidth = column.width - 10;
            const words = text.split(/\s+/);
            let line = "";
            let lineY = vAlign === "middle" ? row.y + 12 : textY;
            for (const word of words) {
              const test = line ? `${line} ${word}` : word;
              if (measureTextWidth(ctx, fontStr, test) > maxWidth && line) {
                ctx.fillText(line, textX, lineY);
                line = word;
                lineY += 14;
                if (lineY > row.y + row.height - 4) break;
              } else {
                line = test;
              }
            }
            if (line && lineY <= row.y + row.height - 4) drawTextWithUnderline(line, textX, lineY, style, style?.align || "left", link);
          } else {
            drawTextWithUnderline(text, textX, textY, style, style?.align || "left", link);
          }
        }
      }
    }

    if (rows.length > 0 && columns.length > 0) {
      const minRow = rows[0].row;
      const maxRow = rows[rows.length - 1].row;
      const minCol = columns[0].col;
      const maxCol = columns[columns.length - 1].col;

      for (const m of currentMerges) {
        const mergeKey = `${m.startRow}:${m.startCol}`;
        if (!drawnMerges.has(mergeKey)) {
          if (m.endRow >= minRow && m.startRow <= maxRow && m.endCol >= minCol && m.startCol <= maxCol) {
            drawnMerges.add(mergeKey);
            renderMergedCell(m);
          }
        }
      }

      // Comment markers: red corner triangle on cells with an open comment.
      const openComments = new Set(
        cellComments()
          .filter((comment) => !comment.resolved)
          .map((comment) => `${comment.row}:${comment.col}`),
      );
      if (openComments.size > 0) {
        for (const row of rows) {
          for (const column of columns) {
            if (!openComments.has(`${row.row}:${column.col}`)) continue;
            const cx = column.x + column.width;
            const cy = row.y;
            ctx.fillStyle = "#e74c3c";
            ctx.beginPath();
            ctx.moveTo(cx - 7, cy);
            ctx.lineTo(cx, cy);
            ctx.lineTo(cx, cy + 7);
            ctx.closePath();
            ctx.fill();
          }
        }
      }
    }

    // Paint frozen panes over the scrolling grid. The overlay keeps the
    // frozen headers and cells anchored while the rest of the canvas moves.
    const frozenRows = Math.min(freezeRows(), 100000);
    const frozenCols = Math.min(freezeCols(), 1000);
    const frozenWidth = offsetForColumn(frozenCols + 1);
    const frozenHeight = offsetForRow(frozenRows + 1);
    if (frozenRows || frozenCols) {
      ctx.fillStyle = "#ffffff";
      ctx.fillRect(headerColWidth, headerRowHeight, frozenWidth, frozenHeight);
      ctx.fillRect(headerColWidth, 0, frozenWidth, headerRowHeight);
      ctx.fillRect(0, headerRowHeight, headerColWidth, frozenHeight);

      for (let c = 1; c <= frozenCols; c++) {
        const x = headerColWidth + offsetForColumn(c);
        const widthForColumn = widthAt(c);
        ctx.strokeStyle = "#e2e8f0";
        ctx.beginPath();
        ctx.moveTo(x, 0);
        ctx.lineTo(x, headerRowHeight + frozenHeight);
        ctx.stroke();
        ctx.fillStyle = "#475569";
        ctx.textAlign = "center";
        ctx.textBaseline = "middle";
        ctx.fillText(getColName(c), x + widthForColumn / 2, headerRowHeight / 2);
        for (let r = 1; r <= frozenRows; r++) {
          const y = headerRowHeight + offsetForRow(r);
          const heightForRow = heightAt(r);
          const value = data[`${r}:${c}`];
          ctx.strokeStyle = "#e2e8f0";
          ctx.beginPath();
          ctx.moveTo(x, y);
          ctx.lineTo(x + widthForColumn, y);
          ctx.stroke();
          if (value) {
            const cellStyle = value.style;
            drawCellImage(cellStyle?.image, x, y, widthForColumn, heightForRow);
            ctx.fillStyle = cellStyle?.fontColor || "#0f172a";
            ctx.font = cellFont(cellStyle, r);
            ctx.textAlign = "left";
            drawTextWithUnderline(value.display, x + 6, y + heightForRow / 2, cellStyle, "left");
          }
        }
      }
      for (let r = 1; r <= frozenRows; r++) {
        const y = headerRowHeight + offsetForRow(r);
        const heightForRow = heightAt(r);
        ctx.strokeStyle = "#e2e8f0";
        ctx.beginPath();
        ctx.moveTo(0, y);
        ctx.lineTo(headerColWidth + frozenWidth, y);
        ctx.stroke();
        ctx.fillStyle = "#475569";
        ctx.textAlign = "center";
        ctx.textBaseline = "middle";
        ctx.fillText(r.toString(), headerColWidth / 2, y + heightForRow / 2);
      }
      ctx.strokeStyle = "#94a3b8";
      ctx.lineWidth = 2;
      if (frozenCols) ctx.strokeRect(headerColWidth + frozenWidth - 1, 0, 1, height);
      if (frozenRows) ctx.strokeRect(0, headerRowHeight + frozenHeight - 1, width, 1);
    }

    // Selection border
    const sel = activeCell();
    const anchor = selectionAnchor();
    const startRow = Math.min(anchor.row, sel.row);
    const endRow = Math.max(anchor.row, sel.row);
    const startCol = Math.min(anchor.col, sel.col);
    const endCol = Math.max(anchor.col, sel.col);
    const selX = headerColWidth - scrollLeft() + offsetForColumn(startCol);
    const selY = headerRowHeight - scrollTop() + offsetForRow(startRow);
    const selWidth = offsetForColumn(endCol + 1) - offsetForColumn(startCol);
    const selHeight = offsetForRow(endRow + 1) - offsetForRow(startRow);
    ctx.strokeStyle = "#16a34a";
    ctx.lineWidth = 2;
    ctx.strokeRect(selX, selY, selWidth, selHeight);

    // Fill handle square
    ctx.fillStyle = "#16a34a";
    ctx.fillRect(selX + selWidth - 4, selY + selHeight - 4, 6, 6);
  };

  onMount(() => {
    const resizeCanvas = () => {
      if (containerRef && canvasRef) {
        const dpr = window.devicePixelRatio || 1;
        const cssW = containerRef.clientWidth;
        const cssH = containerRef.clientHeight;
        canvasRef.width = Math.round(cssW * dpr);
        canvasRef.height = Math.round(cssH * dpr);
        canvasRef.style.width = `${cssW}px`;
        canvasRef.style.height = `${cssH}px`;
        const ctx = canvasRef.getContext("2d");
        if (ctx) ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
        clearTextMeasureCache();
        markGridDirtyFull();
        drawGrid();
      }
    };
    resizeCanvas();
    window.addEventListener("resize", resizeCanvas);
    const previousPerf = import.meta.env.DEV ? window.__perf : undefined;
    let installedPerf: Window["__perf"] = undefined;
    if (import.meta.env.DEV) {
      installedPerf = {
        measureGrid: (frames = 60) => {
          const count = Math.max(1, Math.min(600, Math.floor(frames)));
          const started = performance.now();
          for (let frame = 0; frame < count; frame += 1) drawGrid();
          const durationMs = performance.now() - started;
          const averageFrameMs = durationMs / count;
          return {
            frames: count,
            durationMs,
            averageFrameMs,
            fps: durationMs > 0 ? (count * 1000) / durationMs : Infinity,
          };
        },
      };
      window.__perf = installedPerf;
    }
    onCleanup(() => window.removeEventListener("resize", resizeCanvas));
    onCleanup(() => {
      if (import.meta.env.DEV && window.__perf === installedPerf) {
        window.__perf = previousPerf;
      }
    });
  });

  const handleCanvasClick = (e: MouseEvent) => {
    if (fillDragStart() || suppressNextClick()) {
      setSuppressNextClick(false);
      return;
    }
    containerRef?.focus();
    const rect = canvasRef.getBoundingClientRect();
    const localX = e.clientX - rect.left;
    const localY = e.clientY - rect.top;
    const x = localX + scrollLeft();
    const y = localY + scrollTop();

    // AutoFilter header chevron click
    if (autoFilterEnabled() && localY <= headerRowHeight && localX > headerColWidth) {
      const col = columnAtOffset(x - headerColWidth).col;
      const bounds = filterBounds();
      if (bounds && col >= bounds.startCol && col <= bounds.endCol) {
        openFilterForColumn(col);
        return;
      }
    }

    if (x > headerColWidth && y > headerRowHeight) {
      setFilterDropdownCol(null);
      let col = columnAtOffset(x - headerColWidth).col;
      let row = rowAtOffset(y - headerRowHeight).row;
      const merge = findMerge(row, col);
      if (merge) {
        row = merge.startRow;
        col = merge.startCol;
      }
      setActiveCell({ row, col });
      if (e.shiftKey) {
        // Preserve the anchor for a rectangular Shift-click extension.
      } else {
        setSelectionAnchor({ row, col });
      }
      setEditing(false);

      const key = `${row}:${col}`;
      const cell = cellsData()[key];
      const val = cell ? cell.raw : "";
      setFormulaValue(val);

      if (props.onCellInfoChange) {
        emitCellInfo(row, col);
      }
      drawGrid();
    }
  };

  const cellAtPoint = (event: MouseEvent) => {
    const rect = canvasRef.getBoundingClientRect();
    const x = event.clientX - rect.left + scrollLeft();
    const y = event.clientY - rect.top + scrollTop();
    if (x <= headerColWidth || y <= headerRowHeight) return null;
    return {
      row: rowAtOffset(y - headerRowHeight).row,
      col: columnAtOffset(x - headerColWidth).col,
    };
  };

  const isFillHandlePoint = (event: MouseEvent) => {
    const rect = canvasRef.getBoundingClientRect();
    const x = event.clientX - rect.left;
    const y = event.clientY - rect.top;
    const selection = activeCell();
    const anchor = selectionAnchor();
    const endCol = Math.max(selection.col, anchor.col);
    const endRow = Math.max(selection.row, anchor.row);
    const handleX = headerColWidth - scrollLeft() + offsetForColumn(endCol + 1);
    const handleY = headerRowHeight - scrollTop() + offsetForRow(endRow + 1);
    return Math.abs(x - handleX) <= 10 && Math.abs(y - handleY) <= 10;
  };

  const finishFillDrag = async (event: MouseEvent) => {
    const start = fillDragStart();
    if (!start) return;
    setFillDragStart(null);
    const target = cellAtPoint(event);
    if (!target || (target.row === start.row && target.col === start.col)) {
      drawGrid();
      return;
    }
    pushHistory();
    const next = await commands.fillSeries(makeWorkbook(cellsData()), activeSheetIndex(), start.row, start.col, target.row, target.col);
    loadSheet(next.sheets?.[activeSheetIndex()]);
    props.onChange?.(next);
  };

  const startDimensionDrag = (event: PointerEvent) => {
    const rect = canvasRef.getBoundingClientRect();
    const x = event.clientX - rect.left + scrollLeft();
    const y = event.clientY - rect.top + scrollTop();
    if (y <= headerRowHeight && x > headerColWidth) {
      const hit = columnAtOffset(x - headerColWidth);
      if (widthAt(hit.col) - hit.remainder <= 7) {
        setDimensionDrag({ axis: "column", index: hit.col, pointer: event.clientX, size: widthAt(hit.col) });
        return true;
      }
    }
    if (x <= headerColWidth && y > headerRowHeight) {
      const hit = rowAtOffset(y - headerRowHeight);
      if (heightAt(hit.row) - hit.remainder <= 7) {
        setDimensionDrag({ axis: "row", index: hit.row, pointer: event.clientY, size: heightAt(hit.row) });
        return true;
      }
    }
    return false;
  };

  const updateDimensionDrag = (event: PointerEvent) => {
    const drag = dimensionDrag();
    if (!drag) return;
    const delta = (drag.axis === "column" ? event.clientX : event.clientY) - drag.pointer;
    const size = Math.max(18, Math.min(320, drag.size + delta));
    if (drag.axis === "column") setColumnWidths({ ...columnWidths(), [drag.index]: size });
    else setRowHeights({ ...rowHeights(), [drag.index]: size });
    props.onChange?.(makeWorkbook(cellsData()));
    drawGrid();
  };

  const moveActiveCell = (dRow: number, dCol: number, extend = false) => {
    selectCell(activeCell().row + dRow, activeCell().col + dCol, extend);
  };

  const selectCell = (row: number, col: number, extend = false) => {
    const next = { row: Math.max(1, row), col: Math.max(1, col) };
    setActiveCell(next);
    if (!extend) setSelectionAnchor(next);
    const viewportWidth = containerRef?.clientWidth || 800;
    const viewportHeight = containerRef?.clientHeight || 400;
    const nextColStart = offsetForColumn(next.col);
    const nextColEnd = offsetForColumn(next.col + 1);
    if (nextColStart < scrollLeft()) setScrollLeft(nextColStart);
    else if (nextColEnd > scrollLeft() + viewportWidth - headerColWidth) {
      setScrollLeft(Math.max(0, nextColEnd - viewportWidth + headerColWidth));
    }
    const nextRowStart = offsetForRow(next.row);
    const nextRowEnd = offsetForRow(next.row + 1);
    if (nextRowStart < scrollTop()) setScrollTop(nextRowStart);
    else if (nextRowEnd > scrollTop() + viewportHeight - headerRowHeight) {
      setScrollTop(Math.max(0, nextRowEnd - viewportHeight + headerRowHeight));
    }
    setFormulaValue(cellsData()[`${next.row}:${next.col}`]?.raw || "");
    emitCellInfo(next.row, next.col);
    drawGrid();
  };

  let commitInFlight: Promise<void> | null = null;

  const commitCellEdit = async (val: string) => {
    if (commitInFlight) {
      // Coalesce: the most recent value wins when the previous IPC completes.
      await commitInFlight.catch(() => undefined);
    }
    const { row, col } = activeCell();
    const key = `${row}:${col}`;
    if (cellsData()[key]?.spill) {
      showToast("Spill results are read-only; edit the originating formula instead.");
      setEditing(false);
      return;
    }
    const newMap = { ...cellsData() };
    if (!val.trim()) {
      delete newMap[key];
    } else {
      const display = val.startsWith("=") ? "#EVAL..." : val;
      newMap[key] = { ...(newMap[key] || {}), raw: val, display };
    }
    pushHistory();
    setCellsData(newMap);
    if (newMap[key]?.style?.wrap) autoHeightForWrappedRows(newMap, [row]);
    setEditing(false);
    markSelectionDirty();
    drawGrid();

    const workbook = makeWorkbook(newMap);
    const run = (async () => {
      try {
        const recalculated = await commands.setWorkbookCellValue(
          workbook,
          activeSheetIndex(),
          row,
          col,
          val,
        );
        const active = recalculated.sheets?.[activeSheetIndex()];
        if (active) loadSheet(active);
        props.onChange?.(recalculated);
      } catch (e) {
        props.onChange?.(workbook);
      }
    })();
    commitInFlight = run;
    await run;
    if (commitInFlight === run) commitInFlight = null;
  };

  const importCsv = async () => {
    const imported = await props.onImportCsv?.();
    if (imported) {
      setSheets(imported.sheets.map((sheet: any) => ({ id: sheet.id, name: sheet.name })));
      const index = imported.activeSheetIndex || 0;
      setActiveSheetIndex(index);
      loadSheet(imported.sheets[index]);
      props.onChange?.(imported);
    }
  };

  const emitFreezeChange = async (rows: number, cols: number) => {
    const next = await commands.setFreezeCmd(
      makeWorkbook(cellsData()),
      activeSheetIndex(),
      rows,
      cols,
    );
    setFreezeRows(rows);
    setFreezeCols(cols);
    props.onChange?.(next);
  };

  const freezeFromSelection = async () => {
    const cell = activeCell();
    await emitFreezeChange(cell.row, cell.col);
    markGridDirtyFull();
    drawGrid();
  };

  const unfreezePanes = async () => {
    await emitFreezeChange(0, 0);
    markGridDirtyFull();
    drawGrid();
  };

  const applyNumberFormat = (value: NumberFormatValue) => {
    updateActiveStyle({
      format: value.format,
      decimals: value.decimals,
    });
  };

  const applyValidation = (validation: ListValidation | null) => {
    const bounds = selectedBounds();
    const next = { ...cellsData() };
    for (let row = bounds.startRow; row <= bounds.endRow; row += 1) {
      for (let col = bounds.startCol; col <= bounds.endCol; col += 1) {
        const key = `${row}:${col}`;
        const current = next[key] || { raw: "", display: "" };
        const style = { ...current.style };
        if (validation) style.validation = validation;
        else delete style.validation;
        next[key] = { ...current, style };
      }
    }
    pushHistory();
    setCellsData(next);
    props.onChange?.(makeWorkbook(next));
    markSelectionDirty();
    drawGrid();
  };

  const addConditionalFormatRule = () => {
    const type = conditionalFormatType();
    const rule: ConditionalFormattingRule = {
      range: selectedBounds(),
      type,
      value: conditionalFormatValue(),
      style: { bgColor: conditionalFormatColor() },
      ...(type === "colorScale"
        ? { scaleColors: [conditionalFormatColor(), "#fef08a", "#86efac"] }
        : {}),
    };
    setConditionalFormatting((previous) => [...previous, rule]);
    props.onChange?.(makeWorkbook(cellsData()));
    markGridDirtyFull();
    drawGrid();
  };

  const removeConditionalFormatRule = (index: number) => {
    setConditionalFormatting((previous) => previous.filter((_, ruleIndex) => ruleIndex !== index));
    props.onChange?.(makeWorkbook(cellsData()));
    markGridDirtyFull();
    drawGrid();
  };

  const createPivotTable = () => {
    const bounds = selectedBounds();
    const columnField = pivotColumnField();
    const config: PivotTableConfig = {
      id: `pivot-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`,
      sourceRange: bounds,
      rowField: pivotRowField(),
      columnField: columnField === "" ? undefined : columnField,
      valueField: pivotValueField(),
      aggregation: pivotAggregation(),
      outputStartRow: bounds.endRow + 2,
      outputStartCol: bounds.startCol,
    };
    const slicerMatch = (row: number) => rowMatchesSlicers(cellsData(), row, slicers());
    const result = config.columnField != null
      ? buildMultiFieldPivotTable(
          cellsData(),
          {
            sourceRange: config.sourceRange,
            rowField: config.rowField,
            columnField: config.columnField,
            valueField: config.valueField,
            aggregation: config.aggregation,
            outputStartRow: config.outputStartRow,
            outputStartCol: config.outputStartCol,
          },
          slicerMatch,
        )
      : buildPivotTable(cellsData(), config, slicerMatch);
    if (result.warning) {
      showToast(result.warning, "warning");
      return;
    }
    config.outputRowCount = result.rowCount;
    config.outputColCount = "colCount" in result ? (result as { colCount: number }).colCount : 2;
    const next = { ...cellsData(), ...result.cells };
    pushHistory();
    setCellsData(next);
    setPivotTables((previous) => [...previous, config]);
    props.onChange?.(makeWorkbook(next));
    markGridDirtyFull();
    drawGrid();
  };

  const removePivotTable = (id: string) => {
    const target = pivotTables().find((pivot) => pivot.id === id);
    if (!target) return;
    const next = { ...cellsData() };
    clearPivotOutput(next, target);
    setPivotTables((previous) => previous.filter((pivot) => pivot.id !== id));
    setCellsData(next);
    props.onChange?.(makeWorkbook(next));
    markGridDirtyFull();
    drawGrid();
  };

  const captureScenario = () => {
    const bounds = selectedBounds();
    const name = scenarioName().trim() || `Scenario ${scenarios().length + 1}`;
    let scenario: ScenarioConfig;
    try {
      scenario = captureScenarioRange(
        cellsData(),
        bounds,
        `scenario-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`,
        name,
      );
    } catch (error) {
      showToast(error instanceof Error ? error.message : "Could not capture scenario.", "warning");
      return;
    }
    pushHistory();
    setScenarios((previous) => [...previous, scenario]);
    setScenarioName("");
    props.onChange?.(makeWorkbook(cellsData()));
    showToast(`Saved ${name}`, "success");
  };

  const applyScenario = async (scenario: ScenarioConfig) => {
    let next: Record<string, GridCell>;
    try {
      next = applyScenarioOverrides(cellsData(), scenario);
    } catch (error) {
      showToast(error instanceof Error ? error.message : "Could not apply scenario.", "warning");
      return;
    }
    pushHistory();
    setCellsData(next);
    markGridDirtyFull();
    try {
      const recalculated = await commands.recalculateWorkbook(makeWorkbook(next));
      loadSheet(recalculated.sheets?.[activeSheetIndex()]);
      props.onChange?.(recalculated);
    } catch {
      props.onChange?.(makeWorkbook(next));
      drawGrid();
    }
  };

  const deleteScenario = (id: string) => {
    pushHistory();
    setScenarios((previous) => previous.filter((scenario) => scenario.id !== id));
    props.onChange?.(makeWorkbook(cellsData()));
  };

  const addSlicer = () => {
    const bounds = selectedBounds();
    try {
      const slicer = createSlicer(
        cellsData(),
        bounds,
        slicerColumn(),
        slicerTitle(),
        `slicer-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`,
      );
      if (slicerValues(cellsData(), slicer).length === 0) {
        showToast("Slicer source must contain at least one value.", "warning");
        return;
      }
      pushHistory();
      setSlicers((previous) => [...previous, slicer]);
      setSlicerTitle("");
      props.onChange?.(makeWorkbook(cellsData()));
      markGridDirtyFull();
      drawGrid();
    } catch (error) {
      showToast(error instanceof Error ? error.message : "Could not create slicer.", "warning");
    }
  };

  const updateSlicerSelection = (id: string, value: string) => {
    const target = slicers().find((slicer) => slicer.id === id);
    if (!target) return;
    const next = toggleSlicerValue(target, value, slicerValues(cellsData(), target));
    pushHistory();
    const nextSlicers = slicers().map((slicer) => slicer.id === id ? next : slicer);
    setSlicers(nextSlicers);
    const nextCells = refreshPivotCells(cellsData(), nextSlicers);
    setCellsData(nextCells);
    props.onChange?.(makeWorkbook(nextCells));
    markGridDirtyFull();
    drawGrid();
  };

  const clearSlicerSelection = (id: string) => {
    const target = slicers().find((slicer) => slicer.id === id);
    if (!target || target.selectedValues.length === 0) return;
    pushHistory();
    const nextSlicers = slicers().map((slicer) => slicer.id === id ? { ...slicer, selectedValues: [] } : slicer);
    setSlicers(nextSlicers);
    const nextCells = refreshPivotCells(cellsData(), nextSlicers);
    setCellsData(nextCells);
    props.onChange?.(makeWorkbook(nextCells));
    markGridDirtyFull();
    drawGrid();
  };

  const removeSlicer = (id: string) => {
    if (!slicers().some((slicer) => slicer.id === id)) return;
    pushHistory();
    setSlicers((previous) => previous.filter((slicer) => slicer.id !== id));
    props.onChange?.(makeWorkbook(cellsData()));
    markGridDirtyFull();
    drawGrid();
  };

  // —— Cell comments (offline review) ————————————————
  const addCommentToActiveCell = () => {
    const cell = activeCell();
    const made = makeCellComment(cell.row, cell.col, commentDraft(), props.authorName?.trim() || "You", cellComments());
    if ("error" in made) {
      showToast(made.error, "warning");
      return;
    }
    pushHistory();
    setCellComments(upsertCellComment(cellComments(), made.comment));
    setCommentDraft("");
    props.onChange?.(makeWorkbook(cellsData()));
    showToast("Comment added", "success");
  };

  const toggleCommentResolved = (id: string) => {
    const target = cellComments().find((comment) => comment.id === id);
    if (!target) return;
    pushHistory();
    setCellComments(setCellCommentResolved(cellComments(), id, !target.resolved));
    props.onChange?.(makeWorkbook(cellsData()));
  };

  const deleteComment = (id: string) => {
    if (!cellComments().some((comment) => comment.id === id)) return;
    pushHistory();
    setCellComments(removeCellComment(cellComments(), id));
    props.onChange?.(makeWorkbook(cellsData()));
  };

  const activeNumberFormat = (): NumberFormatValue => ({
    format: activeStyle()?.format || "general",
    decimals: activeStyle()?.decimals ?? 2,
  });

  const activeValidation = (): ListValidation | null => activeStyle()?.validation ?? null;

  const insertRowsAt = async (atRow: number, count = 1) => {
    pushHistory();
    const next = await commands.insertRowsCmd(
      makeWorkbook(cellsData()),
      activeSheetIndex(),
      atRow,
      count,
    );
    loadSheet(next.sheets[activeSheetIndex()]);
    props.onChange?.(next);
    drawGrid();
  };

  const deleteRowsAt = async (atRow: number, count = 1) => {
    pushHistory();
    const next = await commands.deleteRowsCmd(
      makeWorkbook(cellsData()),
      activeSheetIndex(),
      atRow,
      count,
    );
    loadSheet(next.sheets[activeSheetIndex()]);
    props.onChange?.(next);
    drawGrid();
  };

  const insertColsAt = async (atCol: number, count = 1) => {
    pushHistory();
    const next = await commands.insertColsCmd(
      makeWorkbook(cellsData()),
      activeSheetIndex(),
      atCol,
      count,
    );
    loadSheet(next.sheets[activeSheetIndex()]);
    props.onChange?.(next);
    drawGrid();
  };

  const deleteColsAt = async (atCol: number, count = 1) => {
    pushHistory();
    const next = await commands.deleteColsCmd(
      makeWorkbook(cellsData()),
      activeSheetIndex(),
      atCol,
      count,
    );
    loadSheet(next.sheets[activeSheetIndex()]);
    props.onChange?.(next);
    drawGrid();
  };

  const fillDown = async () => {
    const cell = activeCell();
    pushHistory();
    const next = await commands.fillSeries(makeWorkbook(cellsData()), activeSheetIndex(), cell.row, cell.col, cell.row + 5, cell.col);
    loadSheet(next.sheets[activeSheetIndex()]);
    props.onChange?.(next);
  };

  const sortByActiveColumn = async (ascending = true) => {
    const bounds = selectedBounds();
    pushHistory();
    const next = await commands.sortRange(
      makeWorkbook(cellsData()),
      activeSheetIndex(),
      bounds.startRow,
      bounds.endRow,
      bounds.startCol,
      bounds.endCol,
      activeCell().col,
      ascending,
    );
    loadSheet(next.sheets[activeSheetIndex()]);
    props.onChange?.(next);
  };

  const applyMultiSort = async () => {
    const bounds = selectedBounds();
    const keys = sortKeys().filter((k) => k.col >= bounds.startCol && k.col <= bounds.endCol);
    if (!keys.length) return;
    pushHistory();
    const workbook = await commands.sortWorkbookRangeMulti(
      makeWorkbook(cellsData()),
      activeSheetIndex(),
      bounds.startRow,
      bounds.endRow,
      bounds.startCol,
      bounds.endCol,
      keys.map((k) => [k.col, k.ascending] as [number, boolean]),
    );
    loadSheet(workbook.sheets[activeSheetIndex()]);
    props.onChange?.(workbook);
    setSortDialogOpen(false);
  };

  const toggleAutoFilter = () => {
    const next = !autoFilterEnabled();
    setAutoFilterEnabled(next);
    setFilterOpen(next);
    setFilterDropdownCol(null);
    if (!next) {
      setFilterQuery("");
      setColumnFilters({});
      setFilterRange(null);
    } else {
      const bounds = selectedBounds();
      const region =
        bounds.startRow === bounds.endRow && bounds.startCol === bounds.endCol
          ? usedRegion()
          : bounds;
      setFilterRange(region);
      setColumnFilters({});
    }
    drawGrid();
    queueMicrotask(() => props.onChange?.(makeWorkbook(cellsData())));
  };

  const setColumnFilterAll = (col: number, selectAll: boolean) => {
    const all = uniqueColumnValues(col);
    const next = { ...columnFilters() };
    if (selectAll) delete next[col];
    else next[col] = [];
    setColumnFilters(next);
    drawGrid();
    queueMicrotask(() => props.onChange?.(makeWorkbook(cellsData())));
  };

  const printSheet = () => {
    const bounds =
      printMode() === "selection"
        ? selectedBounds()
        : usedRegion();
    const rows: string[] = [];
    for (let r = bounds.startRow; r <= bounds.endRow; r += 1) {
      if (!rowMatchesFilter(r) && printMode() === "sheet") continue;
      const cells: string[] = [];
      for (let c = bounds.startCol; c <= bounds.endCol; c += 1) {
        const text = (cellsData()[`${r}:${c}`]?.display || "").replace(/</g, "&lt;");
        cells.push(`<td>${text}</td>`);
      }
      rows.push(`<tr>${cells.join("")}</tr>`);
    }
    const html = `<!doctype html><html><head><title>Print Sheet</title>
<style>
  body{font:12px Liberation Sans,sans-serif;margin:12px}
  table{border-collapse:collapse;width:${printFitWidth() ? "100%" : "auto"}}
  td,th{border:1px solid #94a3b8;padding:4px 8px;page-break-inside:avoid}
  tr{page-break-inside:avoid}
  @media print{body{margin:0}}
</style></head><body><table>${rows.join("")}</table>
</body></html>`;
    const iframe = document.createElement("iframe");
    iframe.style.position = "absolute";
    iframe.style.width = "0";
    iframe.style.height = "0";
    iframe.style.border = "none";
    document.body.appendChild(iframe);
    const win = iframe.contentWindow;
    if (win) {
      win.document.write(html);
      win.document.close();
      win.focus();
      win.print();
    }
    setTimeout(() => {
      document.body.removeChild(iframe);
    }, 1000);
    setPrintDialogOpen(false);
  };

  const openFilterForColumn = (col: number) => {
    if (!autoFilterEnabled()) return;
    setFilterDropdownCol(filterDropdownCol() === col ? null : col);
  };

  const toggleFilterValue = (col: number, value: string) => {
    const all = uniqueColumnValues(col);
    const current = columnFilters()[col];
    const active = current ?? all;
    const nextSet = new Set(active);
    if (nextSet.has(value)) nextSet.delete(value);
    else nextSet.add(value);
    const nextArr = Array.from(nextSet);
    const next = { ...columnFilters() };
    if (nextArr.length === all.length) delete next[col];
    else next[col] = nextArr;
    setColumnFilters(next);
    drawGrid();
    queueMicrotask(() => props.onChange?.(makeWorkbook(cellsData())));
  };

  const pasteTsv = async () => {
    try {
      // Internal rich paste first: keeps styles and shifts relative formula
      // references exactly like Excel's copy/paste. Falls through to the
      // system TSV clipboard for anything copied outside Redoc.
      const rich = takeRichClipboard();
      if (rich) {
        const origin = activeCell();
        const next = { ...cellsData() };
        for (const [key, cell] of Object.entries(rich.cells)) {
          const [rowOffset, colOffset] = key.split(":").map(Number);
          const targetKey = `${origin.row + rowOffset}:${origin.col + colOffset}`;
          const deltaRow = origin.row - rich.startRow;
          const deltaCol = origin.col - rich.startCol;
          const shiftedRaw = cell.raw.startsWith("=")
            ? shiftFormulaReferences(cell.raw, deltaRow, deltaCol)
            : cell.raw;
          if (!shiftedRaw && !cell.style) {
            delete next[targetKey];
            continue;
          }
          next[targetKey] = {
            raw: shiftedRaw,
            display: shiftedRaw.startsWith("=") ? "#EVAL..." : shiftedRaw,
            style: cell.style ? { ...cell.style } as any : undefined,
          };
        }
        pushHistory();
        setCellsData(next);
        const recalculated = await commands.recalculateWorkbook(makeWorkbook(next));
        loadSheet(recalculated.sheets?.[activeSheetIndex()]);
        props.onChange?.(recalculated);
        return;
      }
      const text = await navigator.clipboard.readText();
      if (!text) return;
      const origin = activeCell();
      const next = { ...cellsData() };
      const rows = parseTsv(text.replace(/\r\n/g, "\n"));
      for (const [rowOffset, line] of rows.entries()) {
        if (!line.length && rowOffset === rows.length - 1) continue;
        for (const [colOffset, raw] of line.entries()) {
          const value = raw.trim();
          const key = `${origin.row + rowOffset}:${origin.col + colOffset}`;
          if (!value) delete next[key];
          else next[key] = { raw: value, display: value.startsWith("=") ? "#EVAL..." : value };
        }
      }
      pushHistory();
      setCellsData(next);
      const recalculated = await commands.recalculateWorkbook(makeWorkbook(next));
      loadSheet(recalculated.sheets?.[activeSheetIndex()]);
      props.onChange?.(recalculated);
    } catch (e) {
      // Clipboard permissions are browser-controlled; leave the sheet unchanged.
    }
  };

  /** Paste values only: formulas become their display text as raw literals. */
  const pasteValuesOnly = async () => {
    try {
      const text = await navigator.clipboard.readText();
      if (!text) return;
      const origin = activeCell();
      const next = { ...cellsData() };
      const pastedKeys: string[] = [];
      for (const [rowOffset, line] of text.replace(/\r\n/g, "\n").split("\n").entries()) {
        if (!line && rowOffset === text.split("\n").length - 1) continue;
        for (const [colOffset, raw] of line.split("\t").entries()) {
          const value = raw.trim();
          const key = `${origin.row + rowOffset}:${origin.col + colOffset}`;
          pastedKeys.push(key);
          if (!value) delete next[key];
          else if (value.startsWith("=")) {
            // Keep formula temporarily so recalculate can produce a display value.
            next[key] = { ...(next[key] || {}), raw: value, display: "#EVAL..." };
          } else {
            next[key] = { ...(next[key] || {}), raw: value, display: value };
          }
        }
      }
      pushHistory();
      setCellsData(next);
      const recalculated = await commands.recalculateWorkbook(makeWorkbook(next));
      const sheet = recalculated.sheets?.[activeSheetIndex()];
      const stripped: Record<string, GridCell> = {};
      for (const [key, cell] of Object.entries(sheet?.cells || {})) {
        const value = cell as any;
        stripped[key] = {
          raw: value.rawValue ?? value.raw ?? "",
          display: value.displayValue ?? value.display ?? value.rawValue ?? "",
          style: value.style ?? undefined,
        };
      }
      // Strip formulas in the pasted region → store display as raw.
      for (const key of pastedKeys) {
        const cell = stripped[key];
        if (!cell) continue;
        if (String(cell.raw).startsWith("=")) {
          const display = cell.display || "";
          stripped[key] = { ...cell, raw: display, display };
        }
      }
      setCellsData(stripped);
      props.onChange?.(makeWorkbook(stripped));
      drawGrid();
    } catch (e) {
      // Clipboard permissions are browser-controlled; leave the sheet unchanged.
    }
  };

  const cellHasData = (row: number, col: number) => {
    const cell = cellsData()[`${row}:${col}`];
    return Boolean(cell && (cell.raw || cell.display));
  };

  const lastUsedCell = () => {
    let maxRow = 1;
    let maxCol = 1;
    for (const key of Object.keys(cellsData())) {
      const [r, c] = key.split(":").map(Number);
      if (!Number.isFinite(r) || !Number.isFinite(c)) continue;
      if (!cellHasData(r, c)) continue;
      if (r > maxRow) maxRow = r;
      if (c > maxCol) maxCol = c;
    }
    return { row: maxRow, col: maxCol };
  };

  /** Jump to the next data edge in the given direction (Excel-like Ctrl+Arrow). */
  const jumpToDataEdge = (dRow: number, dCol: number, extend = false) => {
    const start = activeCell();
    let row = start.row;
    let col = start.col;
    const inBounds = (r: number, c: number) => r >= 1 && c >= 1 && r <= 100000 && c <= 1000;
    const next = (r: number, c: number) => ({ row: r + dRow, col: c + dCol });

    if (!cellHasData(row, col)) {
      // From empty: skip empties until first filled, or stop at last empty before filled fails.
      let cursor = next(row, col);
      while (inBounds(cursor.row, cursor.col) && !cellHasData(cursor.row, cursor.col)) {
        row = cursor.row;
        col = cursor.col;
        cursor = next(row, col);
      }
      if (inBounds(cursor.row, cursor.col) && cellHasData(cursor.row, cursor.col)) {
        selectCell(cursor.row, cursor.col, extend);
      } else {
        selectCell(row, col, extend);
      }
      return;
    }

    // From filled: if next is empty, jump to next filled; else ride along filled until edge.
    const step = next(row, col);
    if (!inBounds(step.row, step.col)) {
      selectCell(row, col, extend);
      return;
    }
    if (!cellHasData(step.row, step.col)) {
      let cursor = step;
      while (inBounds(cursor.row, cursor.col) && !cellHasData(cursor.row, cursor.col)) {
        const ahead = next(cursor.row, cursor.col);
        if (!inBounds(ahead.row, ahead.col)) {
          selectCell(cursor.row, cursor.col, extend);
          return;
        }
        if (cellHasData(ahead.row, ahead.col)) {
          selectCell(ahead.row, ahead.col, extend);
          return;
        }
        cursor = ahead;
      }
      selectCell(cursor.row, cursor.col, extend);
      return;
    }

    let cursor = { row, col };
    while (true) {
      const ahead = next(cursor.row, cursor.col);
      if (!inBounds(ahead.row, ahead.col) || !cellHasData(ahead.row, ahead.col)) {
        selectCell(cursor.row, cursor.col, extend);
        return;
      }
      cursor = ahead;
    }
  };

  const captureSnapshot = (): SheetSnapshot => {
    const data: SheetSnapshot = {
      cells: cellsData(),
      merges: merges(),
      autoFilterEnabled: autoFilterEnabled(),
      filterRange: filterRange() ? { ...filterRange()! } : null,
      columnFilters: columnFilters(),
      columnWidths: columnWidths(),
      rowHeights: rowHeights(),
      chartType: chartType(),
      chartTitle: chartTitle(),
      chartRange: { ...chartRange() },
      conditionalFormatting: conditionalFormatting(),
      pivotTables: pivotTables(),
      scenarios: scenarios(),
      slicers: slicers(),
      comments: cellComments(),
      sheetsMeta: sheets().map((s) => ({ ...s })),
      activeSheetIndex: activeSheetIndex(),
    };
    return structuredClone(data);
  };

  const restoreSnapshot = async (snap: SheetSnapshot) => {
    setMerges(snap.merges);
    setAutoFilterEnabled(snap.autoFilterEnabled);
    setFilterRange(snap.filterRange);
    setColumnFilters(snap.columnFilters);
    setColumnWidths(snap.columnWidths);
    setRowHeights(snap.rowHeights);
    setChartType(snap.chartType);
    setChartTitle(snap.chartTitle);
    setChartRange(snap.chartRange);
    setConditionalFormatting(snap.conditionalFormatting);
    setPivotTables(snap.pivotTables || []);
    setScenarios(snap.scenarios || []);
    setSlicers(snap.slicers || []);
    setCellComments(snap.comments || []);
    setSheets(snap.sheetsMeta);
    setActiveSheetIndex(snap.activeSheetIndex);
    setCellsData(snap.cells);
    const recalculated = await commands.recalculateWorkbook(makeWorkbook(snap.cells));
    loadSheet(recalculated.sheets?.[activeSheetIndex()]);
    props.onChange?.(recalculated);
    drawGrid();
  };

  const { pushHistory, undo, redo } = createSheetHistoryHandlers({
    historyPast,
    setHistoryPast,
    historyFuture,
    setHistoryFuture,
    captureSnapshot,
    restoreSnapshot,
  });

  const restoreHistoryMap = async (next: Record<string, GridCell>) => {
    setCellsData(next);
    const recalculated = await commands.recalculateWorkbook(makeWorkbook(next));
    loadSheet(recalculated.sheets?.[activeSheetIndex()]);
    props.onChange?.(recalculated);
  };

  const updateActiveStyle = (changes: GridCell["style"]) => {
    const bounds = selectedBounds();
    const next = { ...cellsData() };
    for (let row = bounds.startRow; row <= bounds.endRow; row += 1) {
      for (let col = bounds.startCol; col <= bounds.endCol; col += 1) {
        const key = `${row}:${col}`;
        const current = next[key] || { raw: "", display: "" };
        next[key] = { ...current, style: { ...current.style, ...changes } };
      }
    }
    pushHistory();
    setCellsData(next);
    if (changes?.wrap) {
      const rows = Array.from({ length: bounds.endRow - bounds.startRow + 1 }, (_, i) => bounds.startRow + i);
      autoHeightForWrappedRows(next, rows);
    }
    props.onChange?.(makeWorkbook(next));
    markSelectionDirty();
    drawGrid();
  };

  const applyActiveHyperlink = () => {
    const raw = hyperlinkDraft().trim();
    if (!raw) {
      updateActiveStyle({ hyperlink: undefined });
      return;
    }
    const normalized = normalizeSheetHyperlink(raw);
    if (!normalized) {
      showToast("Use an https/http, mailto, tel, or internal cell link.", "error");
      return;
    }
    setHyperlinkDraft(normalized);
    updateActiveStyle({ hyperlink: normalized });
  };

  const insertImage = () => {
    const input = document.createElement("input");
    input.type = "file";
    input.accept = "image/png,image/jpeg,image/gif,image/webp";
    input.onchange = () => {
      const file = input.files?.[0];
      if (!file) return;
      if (file.size > 8 * 1024 * 1024) {
        showToast("Images are limited to 8 MB per cell.", "error");
        return;
      }
      const reader = new FileReader();
      reader.onerror = () => showToast("The image could not be read.", "error");
      reader.onload = () => {
        const source = String(reader.result || "");
        if (!/^data:image\/(?:png|jpe?g|gif|webp);base64,/i.test(source)) {
          showToast("Choose a PNG, JPEG, GIF, or WebP image.", "error");
          return;
        }
        updateActiveStyle({ image: source });
        showToast("Image inserted into the active cell.", "success");
      };
      reader.readAsDataURL(file);
    };
    input.click();
  };

  const activeStyle = () => cellsData()[`${activeCell().row}:${activeCell().col}`]?.style;

  createEffect(() => {
    const cell = cellsData()[`${activeCell().row}:${activeCell().col}`];
    setHyperlinkDraft(cell?.style?.hyperlink || "");
  });

  onMount(() => {
    const onCommand = (event: Event) => {
      const detail = (event as CustomEvent<EditorCommandDetail>).detail;
      if (!detail?.id) return;
      if (detail.id === "find") {
        setFindReplaceMode(false);
        setFindBarOpen(true);
      } else if (detail.id === "find-replace") {
        setFindReplaceMode(true);
        setFindBarOpen(true);
      }
      else if (detail.id === "undo") void undo();
      else if (detail.id === "redo") void redo();
      else if (detail.id === "cut") void cutSelection();
      else if (detail.id === "copy") void copySelection();
      else if (detail.id === "paste") void pasteTsv();
      else if (detail.id === "sort-asc") void sortByActiveColumn(true);
      else if (detail.id === "sort-multi") setSortDialogOpen(true);
      else if (detail.id === "filter") toggleAutoFilter();
      else if (detail.id === "insert-chart") updateChart(chartType() ? null : "bar");
      else if (detail.id === "insert-image") insertImage();
      else if (detail.id === "insert-sheet") addSheet();
      else if (detail.id === "delete-sheet") deleteSheet(activeSheetIndex());
      else if (detail.id === "print") setPrintDialogOpen(true);
      else if (detail.id === "paste-values") void pasteValuesOnly();
      else if (detail.id === "goto-a1") selectCell(1, 1);
      else if (detail.id === "goto-last") {
        const last = lastUsedCell();
        selectCell(last.row, last.col);
      }
      else if (detail.id === "jump-edge") {
        const payload = detail.payload as { dRow?: number; dCol?: number } | undefined;
        jumpToDataEdge(payload?.dRow || 0, payload?.dCol || 0);
      }
      else if (detail.id === "insert-rows") void insertRowsAt(activeCell().row);
      else if (detail.id === "delete-rows") void deleteRowsAt(activeCell().row);
      else if (detail.id === "insert-cols") void insertColsAt(activeCell().col);
      else if (detail.id === "delete-cols") void deleteColsAt(activeCell().col);
      else if (detail.id === "freeze-from-selection") void freezeFromSelection();
      else if (detail.id === "bold") updateActiveStyle({ bold: !activeStyle()?.bold });
      else if (detail.id === "italic") updateActiveStyle({ italic: !activeStyle()?.italic });
      else if (detail.id === "underline") updateActiveStyle({ underline: !activeStyle()?.underline });
      else if (detail.id === "clear-formatting") {
        const bounds = selectedBounds();
        const next = { ...cellsData() };
        for (let row = bounds.startRow; row <= bounds.endRow; row += 1) {
          for (let col = bounds.startCol; col <= bounds.endCol; col += 1) {
            const key = `${row}:${col}`;
            if (!next[key]) continue;
            next[key] = { raw: next[key].raw, display: next[key].display };
          }
        }
        pushHistory();
        setCellsData(next);
        props.onChange?.(makeWorkbook(next));
        drawGrid();
      }
    };
    window.addEventListener(EDITOR_COMMAND, onCommand);
    onCleanup(() => window.removeEventListener(EDITOR_COMMAND, onCommand));
  });

  const insertFormulaPrefix = (text: string) => {
    setFormulaValue(text);
    setEditing(true);
    formulaInputRef?.focus();
    queueMicrotask(() => {
      if (!formulaInputRef) return;
      if (text.endsWith("()")) {
        const pos = text.length - 1;
        formulaInputRef.setSelectionRange(pos, pos);
      } else {
        formulaInputRef.setSelectionRange(text.length, text.length);
      }
    });
  };
  const addSheet = () => {
    const index = sheets().length + 1;
    const newSheet = { id: `sheet-${index}`, name: `Sheet${index}` };
    setSheets([...sheets(), newSheet]);
    const workbook = makeWorkbook(cellsData());
    workbook.sheets = workbook.sheets || [];
    if (!workbook.sheets.some((sheet: any) => sheet.id === newSheet.id)) {
      workbook.sheets.push({
        id: newSheet.id,
        name: newSheet.name,
        cells: {},
        colWidths: {},
        rowHeights: {},
        freezeRows: 0,
        freezeCols: 0,
        charts: [],
        filterQuery: null,
      });
    }
    props.onChange?.(workbook);
  };

  const renameSheet = (index: number) => {
    const current = sheets()[index];
    if (!current) return;
    const nextName = window.prompt("Rename sheet", current.name);
    if (!nextName || nextName === current.name) return;
    const next = [...sheets()];
    next[index] = { ...current, name: nextName };
    setSheets(next);
    
    const workbook = makeWorkbook(cellsData());
    if (workbook.sheets?.[index]) {
      workbook.sheets[index].name = nextName;
    }
    props.onChange?.(workbook);
  };

  const duplicateSheet = (index: number) => {
    const current = sheets()[index];
    if (!current) return;
    const newId = `sheet-${Date.now()}`;
    const newName = `${current.name} Copy`;
    
    const workbook = makeWorkbook(cellsData());
    const sourceSheetData = workbook.sheets?.[index] ? structuredClone(workbook.sheets[index]) : null;
    
    const nextSheets = [...sheets()];
    nextSheets.splice(index + 1, 0, { id: newId, name: newName });
    setSheets(nextSheets);
    
    if (workbook.sheets && sourceSheetData) {
      sourceSheetData.id = newId;
      sourceSheetData.name = newName;
      workbook.sheets.splice(index + 1, 0, sourceSheetData);
    }
    props.onChange?.(workbook);
  };

  const deleteSheet = (index: number) => {
    if (sheets().length <= 1) return;
    const nextSheets = [...sheets()];
    nextSheets.splice(index, 1);
    setSheets(nextSheets);
    
    const workbook = makeWorkbook(cellsData());
    if (workbook.sheets) {
      workbook.sheets.splice(index, 1);
    }
    
    let nextActive = activeSheetIndex();
    if (nextActive >= nextSheets.length) {
      nextActive = nextSheets.length - 1;
    }
    
    if (activeSheetIndex() === index) {
      setActiveSheetIndex(nextActive);
      const source = workbook.sheets?.[nextActive];
      if (source) {
        setFreezeRows(source.freezeRows || 0);
        setFreezeCols(source.freezeCols || 0);
        setColumnWidth(source.colWidths?.[0] || 100);
        setRowHeight(source.rowHeights?.[0] || 26);
        setColumnWidths(source.colWidths || {});
        setRowHeights(source.rowHeights || {});
        loadSheet(source);
      }
      emitCellInfo(activeCell().row, activeCell().col);
      drawGrid();
    } else {
      setActiveSheetIndex(nextActive);
    }
    props.onChange?.(workbook);
  };

  const resizeColumns = (delta: number) => {
    const col = activeCell().col;
    const next = { ...columnWidths(), [col]: Math.max(48, Math.min(320, widthAt(col) + delta)) };
    setColumnWidths(next);
    props.onChange?.(makeWorkbook(cellsData()));
    drawGrid();
  };

  const resizeRows = (delta: number) => {
    const row = activeCell().row;
    const next = { ...rowHeights(), [row]: Math.max(18, Math.min(80, heightAt(row) + delta)) };
    setRowHeights(next);
    props.onChange?.(makeWorkbook(cellsData()));
    drawGrid();
  };

  const findNextCell = () => {
    const query = findQuery().trim();
    if (!query) return;
    const needle = matchCase() ? query : query.toLowerCase();
    const entries = Object.entries(cellsData());
    entries.sort((a, b) => {
      const [rA, cA] = a[0].split(':').map(Number);
      const [rB, cB] = b[0].split(':').map(Number);
      return rA !== rB ? rA - rB : cA - cB;
    });
    const currentKey = `${activeCell().row}:${activeCell().col}`;
    const start = Math.max(0, entries.findIndex(([key]) => key === currentKey) + 1);
    const ordered = [...entries.slice(start), ...entries.slice(0, start)];
    const match = ordered.find(([, cell]) => {
      const haystack = `${cell.raw}\n${cell.display}`;
      return (matchCase() ? haystack : haystack.toLowerCase()).includes(needle);
    });
    if (!match) return;
    const [key, cell] = match;
    const [row, col] = key.split(":").map(Number);
    setActiveCell({ row, col });
    setSelectionAnchor({ row, col });
    setFormulaValue(cell.raw);
    emitCellInfo(row, col);
    drawGrid();
  };

  const replaceCurrentCell = async () => {
    const query = findQuery().trim();
    if (!query) return;
    const { row, col } = activeCell();
    const key = `${row}:${col}`;
    const cell = cellsData()[key];
    if (!cell) return;
    const haystack = `${cell.raw}\n${cell.display}`;
    const matches = matchCase()
      ? haystack.includes(query)
      : haystack.toLowerCase().includes(query.toLowerCase());
    if (!matches) {
      findNextCell();
      return;
    }
    const replacement = replaceWith();
    const nextRaw = cell.raw.includes(query)
      ? (matchCase()
        ? cell.raw.replace(query, replacement)
        : cell.raw.replace(new RegExp(query.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"), "gi"), replacement))
      : cell.raw;
    pushHistory();
    const next = { ...cellsData(), [key]: { ...cell, raw: nextRaw, display: nextRaw.startsWith("=") ? cell.display : nextRaw } };
    setCellsData(next);
    const recalculated = await commands.recalculateWorkbook(makeWorkbook(next));
    loadSheet(recalculated.sheets?.[activeSheetIndex()]);
    props.onChange?.(recalculated);
    setFormulaValue(nextRaw);
    drawGrid();
    findNextCell();
  };

  const replaceAllCells = async () => {
    const query = findQuery().trim();
    if (!query) return;
    pushHistory();
    const next = { ...cellsData() };
    const pattern = matchCase() ? query : new RegExp(query.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"), "gi");
    for (const [key, cell] of Object.entries(next)) {
      const haystack = `${cell.raw}\n${cell.display}`;
      const hit = matchCase()
        ? haystack.includes(query)
        : haystack.toLowerCase().includes(query.toLowerCase());
      if (!hit) continue;
      const newRaw = matchCase()
        ? cell.raw.split(query).join(replaceWith())
        : cell.raw.replace(pattern as RegExp, replaceWith());
      next[key] = {
        ...cell,
        raw: newRaw,
        display: newRaw.startsWith("=") ? cell.display : newRaw,
      };
    }
    setCellsData(next);
    const recalculated = await commands.recalculateWorkbook(makeWorkbook(next));
    loadSheet(recalculated.sheets?.[activeSheetIndex()]);
    props.onChange?.(recalculated);
    drawGrid();
  };

  const addNamedRange = (name: string, rangeStr: string) => {
    const trimmed = name.trim();
    const range = rangeStr.trim();
    if (!trimmed || !range) return;
    const sheetName = sheets()[activeSheetIndex()]?.name || "Sheet1";
    setNamedRanges((prev) => {
      const filtered = prev.filter((nr) => nr.name !== trimmed);
      return [...filtered, { name: trimmed, rangeStr: range, sheet: sheetName }];
    });
    props.onChange?.(makeWorkbook(cellsData()));
  };

  const deleteNamedRange = (name: string) => {
    setNamedRanges((prev) => prev.filter((nr) => nr.name !== name));
    props.onChange?.(makeWorkbook(cellsData()));
  };

  const selectedBounds = () => {
    const cell = activeCell();
    const anchor = selectionAnchor();
    return {
      startRow: Math.min(anchor.row, cell.row),
      endRow: Math.max(anchor.row, cell.row),
      startCol: Math.min(anchor.col, cell.col),
      endCol: Math.max(anchor.col, cell.col),
    };
  };

  const selectionAggregates = () => {
    const bounds = selectedBounds();
    const values: number[] = [];
    for (let row = bounds.startRow; row <= bounds.endRow; row += 1) {
      for (let col = bounds.startCol; col <= bounds.endCol; col += 1) {
        const cell = cellsData()[`${row}:${col}`];
        if (!cell) continue;
        const fromDisplay = Number(cell.display);
        if (Number.isFinite(fromDisplay) && String(cell.display).trim() !== "") {
          values.push(fromDisplay);
          continue;
        }
        const fromRaw = Number(cell.raw);
        if (Number.isFinite(fromRaw) && String(cell.raw).trim() !== "" && !cell.raw.startsWith("=")) {
          values.push(fromRaw);
        }
      }
    }
    if (!values.length) return null;
    const count = values.length;
    const sum = values.reduce((total, value) => total + value, 0);
    const min = Math.min(...values);
    const max = Math.max(...values);
    return { count, min, max, sum, avg: sum / count };
  };

  const emitCellInfo = (row: number, col: number) => {
    const sheetLabel = `Sheet ${activeSheetIndex() + 1} of ${sheets().length}`;
    const cellLabel = `Cell ${getColName(col)}${row}`;
    const agg = selectionAggregates();
    const info = agg
      ? `${sheetLabel} · ${cellLabel} · Count: ${agg.count}; Min: ${agg.min}; Max: ${agg.max}; Average: ${agg.avg.toFixed(2)}; Sum: ${agg.sum}`
      : `${sheetLabel} · ${cellLabel}`;
    props.onCellInfoChange?.(info);
  };

  const selectedTsv = () => {
    const bounds = selectedBounds();
    return Array.from({ length: bounds.endRow - bounds.startRow + 1 }, (_, rowOffset) =>
      Array.from({ length: bounds.endCol - bounds.startCol + 1 }, (_, colOffset) => {
        const raw = cellsData()[`${bounds.startRow + rowOffset}:${bounds.startCol + colOffset}`]?.raw || "";
        if (raw.includes('\n') || raw.includes('\t') || raw.includes('"')) {
          return `"${raw.replace(/"/g, '""')}"`;
        }
        return raw;
      }).join("\t")
    ).join("\n");
  };

  const selectedRichClipboard = (cut: boolean): RichClipboard => {
    const bounds = selectedBounds();
    const cells: RichClipboard["cells"] = {};
    for (let row = bounds.startRow; row <= bounds.endRow; row += 1) {
      for (let col = bounds.startCol; col <= bounds.endCol; col += 1) {
        const cell = cellsData()[`${row}:${col}`];
        if (!cell) continue;
        cells[`${row - bounds.startRow}:${col - bounds.startCol}`] = {
          raw: cell.raw,
          display: cell.display,
          style: cell.style ? { ...cell.style } as Record<string, unknown> : undefined,
        };
      }
    }
    return {
      kind: "sheet",
      startRow: bounds.startRow,
      startCol: bounds.startCol,
      endRow: bounds.endRow,
      endCol: bounds.endCol,
      cells,
      cut,
    };
  };

  const cutSelection = async () => {
    await navigator.clipboard.writeText(selectedTsv()).catch(() => {});
    setRichClipboard(selectedRichClipboard(true));
    const next = { ...cellsData() };
    const bounds = selectedBounds();
    for (let row = bounds.startRow; row <= bounds.endRow; row += 1) {
      for (let col = bounds.startCol; col <= bounds.endCol; col += 1) {
        delete next[`${row}:${col}`];
      }
    }
    pushHistory();
    setCellsData(next);
    const recalculated = await commands.recalculateWorkbook(makeWorkbook(next));
    loadSheet(recalculated.sheets?.[activeSheetIndex()]);
    props.onChange?.(recalculated);
  };

  const copySelection = async () => {
    await navigator.clipboard.writeText(selectedTsv()).catch(() => {});
    setRichClipboard(selectedRichClipboard(false));
  };

  const sidebarPanels = (): SidebarPanel[] => [
    {
      id: "properties",
      title: "Properties",
      icon: <IconProperties />,
      content: (
        <div style={{ display: "flex", "flex-direction": "column", gap: "8px" }}>
          <div>
            <div style={{ "font-weight": "600", color: "var(--text-primary)" }}>
              {getColName(activeCell().col)}{activeCell().row}
            </div>
            <div>Sheet: {sheets()[activeSheetIndex()]?.name || "Sheet1"}</div>
          </div>
          <div>Raw: {cellsData()[`${activeCell().row}:${activeCell().col}`]?.raw || "(empty)"}</div>
          <div>Display: {cellsData()[`${activeCell().row}:${activeCell().col}`]?.display || "(empty)"}</div>
          <div>
            Style: bold={String(!!activeStyle()?.bold)}, italic={String(!!activeStyle()?.italic)},
            underline={String(!!activeStyle()?.underline)}, align={activeStyle()?.align || "left"},
            format={activeStyle()?.format || "general"}
          </div>
          <div>Colors: {activeStyle()?.fontColor || "default"} / {activeStyle()?.bgColor || "none"}</div>
          <div style={{ "border-top": "1px solid var(--border-color)", "padding-top": "8px", "margin-top": "2px" }}>
            <label style={{ display: "flex", "flex-direction": "column", gap: "4px", "font-size": "12px" }}>
              Cell hyperlink
              <input
                class="g-toolbar-input"
                aria-label="Cell hyperlink"
                value={hyperlinkDraft()}
                onInput={(event) => setHyperlinkDraft(event.currentTarget.value)}
                placeholder="https://… or internal:Sheet2!B4"
                style={{ width: "100%" }}
              />
            </label>
            <div style={{ display: "flex", gap: "6px", "margin-top": "6px" }}>
              <button type="button" class="g-toolbar-btn active" onClick={applyActiveHyperlink}>Apply link</button>
              <button type="button" class="g-toolbar-btn" onClick={() => { setHyperlinkDraft(""); applyActiveHyperlink(); }}>Clear</button>
            </div>
          </div>
        </div>
      ),
    },
    {
      id: "styles",
      title: "Styles",
      icon: <IconStyles />,
      content: (
        <div style={{ display: "flex", "flex-direction": "column", gap: "8px" }}>
          <div style={{ "font-weight": "600" }}>Named ranges</div>
          <For each={namedRanges()}>
            {(nr) => (
              <div style={{ display: "flex", gap: "6px", "align-items": "center", "font-size": "12px" }}>
                <span style={{ flex: 1 }}>{nr.name} → {nr.rangeStr}</span>
                <button type="button" class="g-toolbar-btn" onClick={() => deleteNamedRange(nr.name)}>✕</button>
              </div>
            )}
          </For>
          <div style={{ display: "flex", "flex-direction": "column", gap: "4px", "margin-top": "8px" }}>
            <input class="g-toolbar-input" placeholder="Name" aria-label="Named range name" id="nr-name" style={{ width: "100%" }} />
            <input class="g-toolbar-input" placeholder="A1:B5" aria-label="Named range" id="nr-range" style={{ width: "100%" }} />
            <button
              type="button"
              class="g-toolbar-btn"
              style={{ width: "100%" }}
              onClick={() => {
                const nameEl = document.getElementById("nr-name") as HTMLInputElement | null;
                const rangeEl = document.getElementById("nr-range") as HTMLInputElement | null;
                if (nameEl && rangeEl) {
                  addNamedRange(nameEl.value, rangeEl.value);
                  nameEl.value = "";
                  rangeEl.value = "";
                }
              }}
            >
              Add named range
            </button>
          </div>
          <div style={{ "border-top": "1px solid var(--border-color)", "padding-top": "8px", "margin-top": "4px" }}>
            <div style={{ "font-weight": "600", "margin-bottom": "6px" }}>Conditional formatting</div>
            <div style={{ "font-size": "11px", color: "var(--text-muted)", "margin-bottom": "6px" }}>Applies to the current selection.</div>
            <select
              class="g-toolbar-select"
              aria-label="Conditional formatting rule"
              value={conditionalFormatType()}
              onChange={(event) => setConditionalFormatType(event.currentTarget.value as ConditionalFormattingRule["type"])}
              style={{ width: "100%", "margin-bottom": "5px" }}
            >
              <option value="greaterThan">Greater than</option>
              <option value="lessThan">Less than</option>
              <option value="equalTo">Equal to</option>
              <option value="textContains">Text contains</option>
              <option value="dataBar">Data bar</option>
              <option value="colorScale">Color scale</option>
            </select>
            <input
              class="g-toolbar-input"
              aria-label="Conditional formatting value"
              value={conditionalFormatValue()}
              onInput={(event) => setConditionalFormatValue(event.currentTarget.value)}
              placeholder="Value"
              style={{ width: "100%", "margin-bottom": "5px" }}
            />
            <input
              type="color"
              aria-label="Conditional formatting color"
              value={conditionalFormatColor()}
              onInput={(event) => setConditionalFormatColor(event.currentTarget.value)}
              style={{ width: "100%", height: "28px", "margin-bottom": "5px" }}
            />
            <button type="button" class="g-toolbar-btn" style={{ width: "100%" }} onClick={addConditionalFormatRule}>Add rule</button>
            <For each={conditionalFormatting()}>
              {(rule, index) => (
                <div style={{ display: "flex", "align-items": "center", gap: "5px", "font-size": "11px", "margin-top": "5px" }}>
                  <span style={{ flex: 1, overflow: "hidden", "text-overflow": "ellipsis", "white-space": "nowrap" }}>{rule.type} {rule.value || ""}</span>
                  <button type="button" class="g-toolbar-btn" aria-label={`Remove conditional formatting rule ${index() + 1}`} onClick={() => removeConditionalFormatRule(index())}>✕</button>
                </div>
              )}
            </For>
            <div style={{ "border-top": "1px solid var(--border-color)", "padding-top": "8px", "margin-top": "8px" }}>
              <div style={{ "font-weight": "600", "margin-bottom": "6px" }}>Pivot summary</div>
              <div style={{ "font-size": "11px", color: "var(--text-muted)", "margin-bottom": "6px" }}>Select a source range with a header row, then choose the row and value columns.</div>
              <div style={{ display: "flex", gap: "5px" }}>
                <label style={{ flex: 1, "font-size": "11px" }}>Row column
                  <input type="number" min="1" value={pivotRowField()} onInput={(e) => setPivotRowField(Math.max(1, Number(e.currentTarget.value) || 1))} class="g-toolbar-input" style={{ width: "100%" }} />
                </label>
                <label style={{ flex: 1, "font-size": "11px" }}>Value column
                  <input type="number" min="1" value={pivotValueField()} onInput={(e) => setPivotValueField(Math.max(1, Number(e.currentTarget.value) || 1))} class="g-toolbar-input" style={{ width: "100%" }} />
                </label>
              </div>
              <label style={{ display: "block", "font-size": "11px", "margin-top": "5px" }}>Column column (optional — cross-tab)
                <input
                  type="number"
                  min="1"
                  aria-label="Pivot column field"
                  class="g-toolbar-input"
                  style={{ width: "100%" }}
                  value={pivotColumnField()}
                  onInput={(e) => {
                    const raw = e.currentTarget.value;
                    setPivotColumnField(raw === "" ? "" : Math.max(1, Number(raw) || 1));
                  }}
                />
              </label>
              <select class="g-toolbar-select" aria-label="Pivot aggregation" value={pivotAggregation()} onChange={(e) => setPivotAggregation(e.currentTarget.value as PivotTableConfig["aggregation"])} style={{ width: "100%", "margin-top": "5px" }}>
                <option value="sum">Sum</option>
                <option value="count">Count</option>
                <option value="average">Average</option>
              </select>
              <button type="button" class="g-toolbar-btn" style={{ width: "100%", "margin-top": "5px" }} onClick={createPivotTable}>Create pivot summary below selection</button>
              <For each={pivotTables()}>
                {(pivot) => (
                  <div style={{ display: "flex", "align-items": "center", gap: "5px", "font-size": "11px", "margin-top": "5px" }}>
                    <span style={{ flex: 1 }}>Pivot {pivot.aggregation} · {pivot.outputRowCount || 0} rows</span>
                    <button type="button" class="g-toolbar-btn" aria-label={`Remove pivot ${pivot.id}`} onClick={() => removePivotTable(pivot.id)}>✕</button>
                  </div>
                )}
              </For>
            </div>
            <div style={{ "border-top": "1px solid var(--border-color)", "padding-top": "8px", "margin-top": "8px" }}>
              <div style={{ "font-weight": "600", "margin-bottom": "6px" }}>What-if scenarios</div>
              <div style={{ "font-size": "11px", color: "var(--text-muted)", "margin-bottom": "6px" }}>Capture the selected cells, then apply a saved set of overrides later.</div>
              <input
                class="g-toolbar-input"
                aria-label="Scenario name"
                value={scenarioName()}
                onInput={(event) => setScenarioName(event.currentTarget.value)}
                placeholder={`Scenario ${scenarios().length + 1}`}
                style={{ width: "100%", "margin-bottom": "5px" }}
              />
              <button type="button" class="g-toolbar-btn" style={{ width: "100%" }} onClick={captureScenario}>Capture selected cells</button>
              <For each={scenarios()}>
                {(scenario) => (
                  <div style={{ display: "flex", "align-items": "center", gap: "5px", "font-size": "11px", "margin-top": "5px" }}>
                    <span style={{ flex: 1, overflow: "hidden", "text-overflow": "ellipsis", "white-space": "nowrap" }}>{scenario.name} · {scenario.changes.length} cells</span>
                    <button type="button" class="g-toolbar-btn" aria-label={`Apply scenario ${scenario.name}`} onClick={() => void applyScenario(scenario)}>Apply</button>
                    <button type="button" class="g-toolbar-btn" aria-label={`Delete scenario ${scenario.name}`} onClick={() => deleteScenario(scenario.id)}>✕</button>
                  </div>
                )}
              </For>
            </div>
            <div style={{ "border-top": "1px solid var(--border-color)", "padding-top": "8px", "margin-top": "8px" }}>
              <div style={{ "font-weight": "600", "margin-bottom": "6px" }}>Slicers</div>
              <div style={{ "font-size": "11px", color: "var(--text-muted)", "margin-bottom": "6px" }}>Create a reusable value filter for the selected table range. An empty selection shows all values.</div>
              <div style={{ display: "flex", gap: "5px" }}>
                <label style={{ flex: 1, "font-size": "11px" }}>Column
                  <input
                    type="number"
                    min="1"
                    value={slicerColumn()}
                    onInput={(event) => setSlicerColumn(Math.max(1, Number(event.currentTarget.value) || 1))}
                    class="g-toolbar-input"
                    style={{ width: "100%" }}
                  />
                </label>
                <label style={{ flex: 2, "font-size": "11px" }}>Title
                  <input
                    class="g-toolbar-input"
                    aria-label="Slicer title"
                    value={slicerTitle()}
                    onInput={(event) => setSlicerTitle(event.currentTarget.value)}
                    placeholder="Column filter"
                    style={{ width: "100%" }}
                  />
                </label>
              </div>
              <button type="button" class="g-toolbar-btn" style={{ width: "100%", "margin-top": "5px" }} onClick={addSlicer}>Create slicer from selection</button>
              <For each={slicers()}>
                {(slicer) => {
                  const values = () => slicerValues(cellsData(), slicer);
                  return (
                    <div style={{ "border-top": "1px solid var(--border-color)", "margin-top": "7px", "padding-top": "6px" }}>
                      <div style={{ display: "flex", "align-items": "center", gap: "5px" }}>
                        <span style={{ flex: 1, "font-size": "11px", "font-weight": "600" }}>{slicer.title}</span>
                        <button type="button" class="g-toolbar-btn" onClick={() => clearSlicerSelection(slicer.id)}>All</button>
                        <button type="button" class="g-toolbar-btn" aria-label={`Remove slicer ${slicer.title}`} onClick={() => removeSlicer(slicer.id)}>✕</button>
                      </div>
                      <div style={{ display: "flex", "flex-direction": "column", gap: "2px", "max-height": "150px", overflow: "auto", "margin-top": "4px" }}>
                        <For each={values()}>
                          {(value) => (
                            <label style={{ display: "flex", gap: "5px", "align-items": "center", "font-size": "11px" }}>
                              <input
                                type="checkbox"
                                checked={slicer.selectedValues.length === 0 || slicer.selectedValues.includes(value)}
                                onChange={() => updateSlicerSelection(slicer.id, value)}
                              />
                              <span>{value || "(blank)"}</span>
                            </label>
                          )}
                        </For>
                      </div>
                    </div>
                  );
                }}
              </For>
            </div>
            <div style={{ "border-top": "1px solid var(--border-color)", "padding-top": "8px", "margin-top": "8px" }}>
              <div style={{ "font-weight": "600", "margin-bottom": "6px" }}>
                Comments {openCommentCount(cellComments()) > 0 ? `· ${openCommentCount(cellComments())} open` : ""}
              </div>
              <div style={{ "font-size": "11px", color: "var(--text-muted)", "margin-bottom": "6px" }}>
                Comments attach to the active cell and persist with the workbook.
              </div>
              <textarea
                class="g-toolbar-input"
                aria-label="New comment"
                placeholder="Comment on the active cell…"
                value={commentDraft()}
                onInput={(event) => setCommentDraft(event.currentTarget.value)}
                rows={2}
                style={{ width: "100%", resize: "vertical" }}
              />
              <button type="button" class="g-toolbar-btn" style={{ width: "100%", "margin-top": "5px" }} onClick={addCommentToActiveCell}>
                Comment on {getColName(activeCell().col)}{activeCell().row}
              </button>
              <Show when={commentsForCell(cellComments(), activeCell().row, activeCell().col).length > 0}>
                <div style={{ "font-size": "11px", color: "var(--text-muted)", "margin-top": "8px", "margin-bottom": "3px" }}>
                  On this cell
                </div>
              </Show>
              <For each={commentsForCell(cellComments(), activeCell().row, activeCell().col)}>
                {(comment) => (
                  <div style={{ border: "1px solid var(--border-color)", "border-radius": "6px", padding: "5px", "margin-top": "5px" }}>
                    <div style={{ display: "flex", "align-items": "center", gap: "5px" }}>
                      <span style={{ flex: 1, "font-size": "11px", "font-weight": "600" }}>{comment.author}</span>
                      <button
                        type="button"
                        class="g-toolbar-btn"
                        aria-label={comment.resolved ? `Reopen comment ${comment.id}` : `Resolve comment ${comment.id}`}
                        onClick={() => toggleCommentResolved(comment.id)}
                      >
                        {comment.resolved ? "Reopen" : "Resolve"}
                      </button>
                      <button type="button" class="g-toolbar-btn" aria-label={`Delete comment ${comment.id}`} onClick={() => deleteComment(comment.id)}>✕</button>
                    </div>
                    <div style={{ "font-size": "11px", "margin-top": "3px", color: comment.resolved ? "var(--text-muted)" : "var(--text-primary)", "text-decoration": comment.resolved ? "line-through" : "none" }}>
                      {comment.text}
                    </div>
                  </div>
                )}
              </For>
            </div>
          </div>
        </div>
      ),
    },
    {
      id: "gallery",
      title: "Gallery",
      icon: <IconGallery />,
      content: (
        <div style={{ display: "flex", "flex-direction": "column", gap: "8px" }}>
          <label style={{ "font-size": "12px" }}>
            Chart title
            <input
              class="g-toolbar-input"
              aria-label="Chart title"
              value={chartTitle()}
              onInput={(e) => {
                setChartTitle(e.currentTarget.value);
                queueMicrotask(() => props.onChange?.(makeWorkbook(cellsData())));
              }}
              style={{ width: "100%", "margin-top": "4px" }}
            />
          </label>
          <label style={{ "font-size": "12px" }}>
            Type
            <select
              aria-label="Chart type"
              value={chartType() || "bar"}
              onChange={(e) => updateChart(e.currentTarget.value as "bar" | "line" | "pie" | "area" | "scatter" | "doughnut")}
              style={{ width: "100%", "margin-top": "4px" }}
            >
              <option value="bar">Bar</option>
              <option value="line">Line</option>
              <option value="area">Area</option>
              <option value="scatter">Scatter</option>
              <option value="pie">Pie</option>
              <option value="doughnut">Doughnut</option>
            </select>
          </label>
          <div style={{ display: "grid", "grid-template-columns": "1fr 1fr", gap: "6px" }}>
            <label style={{ "font-size": "11px" }}>
              Start row
              <input
                type="number"
                class="g-toolbar-input"
                aria-label="Chart start row"
                value={chartRange().startRow}
                onInput={(e) => setChartRange({ ...chartRange(), startRow: Number(e.currentTarget.value) || 1 })}
                style={{ width: "100%" }}
              />
            </label>
            <label style={{ "font-size": "11px" }}>
              End row
              <input
                type="number"
                class="g-toolbar-input"
                aria-label="Chart end row"
                value={chartRange().endRow}
                onInput={(e) => setChartRange({ ...chartRange(), endRow: Number(e.currentTarget.value) || 1 })}
                style={{ width: "100%" }}
              />
            </label>
            <label style={{ "font-size": "11px" }}>
              Start col
              <input
                type="number"
                class="g-toolbar-input"
                aria-label="Chart start col"
                value={chartRange().startCol}
                onInput={(e) => setChartRange({ ...chartRange(), startCol: Number(e.currentTarget.value) || 1 })}
                style={{ width: "100%" }}
              />
            </label>
            <label style={{ "font-size": "11px" }}>
              End col
              <input
                type="number"
                class="g-toolbar-input"
                aria-label="Chart end col"
                value={chartRange().endCol}
                onInput={(e) => setChartRange({ ...chartRange(), endCol: Number(e.currentTarget.value) || 1 })}
                style={{ width: "100%" }}
              />
            </label>
          </div>
          <button type="button" class="g-toolbar-btn" onClick={() => updateChart(chartType() || "bar")}>
            Use selection as range
          </button>
          <button type="button" class="g-toolbar-btn" onClick={() => updateChart(null)}>
            Remove chart
          </button>
        </div>
      ),
    },
    {
      id: "navigator",
      title: "Navigator",
      icon: <IconNavigator />,
      content: (
        <div style={{ display: "flex", "flex-direction": "column", gap: "4px" }}>
          <For each={sheets()}>
            {(sheet, index) => (
              <button
                type="button"
                class="g-toolbar-btn"
                style={{ width: "100%", "justify-content": "flex-start", height: "22px" }}
                onClick={() => {
                  const workbook = makeWorkbook(cellsData());
                  props.onChange?.(workbook);
                  setActiveSheetIndex(index());
                  const source = workbook.sheets?.[index()];
                  if (source) {
                    setFreezeRows(source.freezeRows || 0);
                    setFreezeCols(source.freezeCols || 0);
                    setColumnWidth(source.colWidths?.[0] || 100);
                    setRowHeight(source.rowHeights?.[0] || 26);
                    setColumnWidths(source.colWidths || {});
                    setRowHeights(source.rowHeights || {});
                    loadSheet(source);
                  }
                  emitCellInfo(activeCell().row, activeCell().col);
                  drawGrid();
                }}
              >
                {sheet.name}
              </button>
            )}
          </For>
        </div>
      ),
    },
    {
      id: "functions",
      title: "Functions",
      icon: <IconFunctions />,
      content: (
        <div style={{ display: "flex", "flex-direction": "column", gap: "2px" }}>
          <For each={FORMULA_FUNCTION_NAMES}>
            {(name) => (
              <button
                type="button"
                class="g-toolbar-btn"
                style={{ width: "100%", "justify-content": "flex-start", height: "22px" }}
                onClick={() => insertFormulaPrefix(`=${name}()`)}
              >
                {name}
              </button>
            )}
          </For>
        </div>
      ),
    },
  ];

  return (
    <div style={{ display: "flex", "flex-direction": "column", height: "100%", background: "var(--bg-surface)", overflow: "hidden" }}>
      {/* Standard toolbar */}
      <ToolbarRow>
        <ToolbarButton title="New" onClick={() => props.onRequestNew?.()}><IconNew /></ToolbarButton>
        <ToolbarButton title="Open" onClick={() => props.onRequestOpen?.()}><IconFolderOpen /></ToolbarButton>
        <ToolbarButton title="Save" onClick={() => props.onRequestSave?.()}><IconSave /></ToolbarButton>
        <ToolbarButton title="Export PDF" onClick={() => props.onRequestExportPdf?.()}><IconPdf /></ToolbarButton>
        <ToolbarButton title="Print" onClick={() => setPrintDialogOpen(true)}><IconPrint /></ToolbarButton>
        <ToolbarSep />
        <ToolbarButton title="Cut" onClick={() => void cutSelection()}><IconCut /></ToolbarButton>
        <ToolbarButton title="Copy" onClick={() => void copySelection()}><IconCopy /></ToolbarButton>
        <ToolbarButton title="Paste" onClick={() => void pasteTsv()}><IconPaste /></ToolbarButton>
        <ToolbarSep />
        <ToolbarButton title="Undo" onClick={() => void undo()} disabled={!historyPast().length}><IconUndo /></ToolbarButton>
        <ToolbarButton title="Redo" onClick={() => void redo()} disabled={!historyFuture().length}><IconRedo /></ToolbarButton>
        <ToolbarSep />
        <ToolbarButton title="Sort Ascending" onClick={() => void sortByActiveColumn(true)}><IconSortAsc /></ToolbarButton>
        <ToolbarButton title="Sort Descending" onClick={() => void sortByActiveColumn(false)}><IconSortDesc /></ToolbarButton>
        <ToolbarButton title="Sort…" onClick={() => setSortDialogOpen(true)}>Sort…</ToolbarButton>
        <ToolbarButton title="AutoFilter" onClick={toggleAutoFilter} active={autoFilterEnabled() || filterOpen() || !!filterQuery()}><IconFilter /></ToolbarButton>
        <Show when={autoFilterEnabled()}>
          <ToolbarButton
            title="Column filter"
            onClick={() => openFilterForColumn(activeCell().col)}
            active={filterDropdownCol() === activeCell().col}
          >
            ▾ {getColName(activeCell().col)}
          </ToolbarButton>
        </Show>
        <ToolbarButton title="Insert Chart" onClick={() => updateChart(chartType() ? null : "bar")} active={!!chartType()}><IconChart /></ToolbarButton>
        <ToolbarSelect
          ariaLabel="Chart type"
          width="72px"
          value={chartType() || "bar"}
          onChange={(v) => updateChart(v as "bar" | "line" | "pie" | "area" | "scatter" | "doughnut")}
          options={[
            { value: "bar", label: "Bar" },
            { value: "line", label: "Line" },
            { value: "area", label: "Area" },
            { value: "scatter", label: "Scatter" },
            { value: "pie", label: "Pie" },
            { value: "doughnut", label: "Doughnut" },
          ]}
        />
        <ToolbarButton title="Insert Image into active cell" onClick={insertImage}><IconImage /></ToolbarButton>
        <ToolbarSep />
        <ToolbarButton title="Insert row" onClick={() => void insertRowsAt(activeCell().row)}>+Row</ToolbarButton>
        <ToolbarButton title="Delete row" onClick={() => void deleteRowsAt(activeCell().row)}>−Row</ToolbarButton>
        <ToolbarButton title="Insert column" onClick={() => void insertColsAt(activeCell().col)}>+Col</ToolbarButton>
        <ToolbarButton title="Delete column" onClick={() => void deleteColsAt(activeCell().col)}>−Col</ToolbarButton>
        <ToolbarSep />
        <ToolbarButton title="Freeze panes at selection" onClick={() => void freezeFromSelection()} active={freezeRows() > 0 || freezeCols() > 0}><IconFreeze /></ToolbarButton>
        <ToolbarButton title="Unfreeze panes" onClick={() => void unfreezePanes()} disabled={freezeRows() === 0 && freezeCols() === 0}>Unfreeze</ToolbarButton>
        <ToolbarButton title="Fill down" onClick={() => void fillDown()}>Fill</ToolbarButton>
        <ToolbarButton title="Import CSV" onClick={() => void importCsv()}>Import</ToolbarButton>
        <ToolbarButton title="Export CSV" onClick={() => props.onExportCsv?.()}>Export</ToolbarButton>
        <ToolbarButton title="Print…" onClick={() => setPrintDialogOpen(true)}><IconPrint /></ToolbarButton>
      </ToolbarRow>

      <SheetToolbar
        {...{
          fontFamily,
          setFontFamily,
          fontSize,
          setFontSize,
          activeStyle,
          updateActiveStyle,
          selectedBounds,
          activeCell,
          setMerges,
          pushHistory,
          makeWorkbook,
          cellsData,
          drawGrid,
          onChange: props.onChange,
          onOpenNumberFormat: () => setNumberFormatOpen(true),
          onOpenValidation: () => setValidationOpen(true),
        }}
      />
      {/* aria-label="Formula input" */}
      <FormulaBar
        {...{
          activeCell,
          formulaValue,
          setFormulaValue,
          getColName,
          insertFormulaPrefix,
          commitCellEdit,
          editing,
          setEditing,
          cellsData,
          cellsBySheet: validationCellsBySheet,
          namedRanges,
          activeSheetName: () => sheets()[activeSheetIndex()]?.name ?? "",
          containerRef,
          formulaInputRef,
        }}
      />
      <div
        data-pane="canvas"
        role="grid"
        aria-label="Spreadsheet grid. Arrow keys move the active cell; Enter or F2 edits; Ctrl+Home jumps to A1."
        aria-rowcount="100000"
        aria-live="polite"
        style={{
          flex: 1,
          display: "flex",
          "min-height": "0",
          overflow: "hidden",
        }}
      >
        <div style={{
          "transform-origin": "top left",
          transform: `scale(${(props.zoomLevel || 100) / 100})`,
          width: `${100 / ((props.zoomLevel || 100) / 100)}%`,
          height: `${100 / ((props.zoomLevel || 100) / 100)}%`,
          display: "flex",
          "flex-direction": "column"
        }}>
          <GridCanvas
          {...{
            containerRef,
            canvasRef,
            scrollTop,
            scrollLeft,
            setScrollTop,
            setScrollLeft,
            rowHeight,
            columnWidth,
            drawGrid,
            markGridDirtyFull,
            activeCell,
            redo,
            undo,
            copySelection,
            cutSelection,
            pasteValuesOnly,
            pasteTsv,
            selectCell,
            moveActiveCell,
            lastUsedCell,
            jumpToDataEdge,
            setEditing,
            setFormulaValue,
            formulaInputRef,
            handleCanvasClick,
            setContextMenu,
            emitEditorCommand,
            isFillHandlePoint,
            setFillDragStart,
            setSuppressNextClick,
            startDimensionDrag,
            updateDimensionDrag,
            fillDragStart,
            finishFillDrag,
            dimensionDrag,
            setDimensionDrag,
            getColName,
            cellsData,
            cellHyperlink,
            onOpenHyperlink: openSheetHyperlink,
            chartType,
            chartTitle: chartTitle(),
            chartTop: () => headerRowHeight - scrollTop() + offsetForRow(chartRange().startRow),
            chartLeft: () => headerColWidth - scrollLeft() + offsetForColumn(chartRange().startCol),
            chartData,
            chartMax,
            pieSlices,
            conditionalFormatting,
          }}
        />
        <Show when={editing()}>
          <input
            type="text"
            spellcheck={props.spellcheckEnabled !== false}
            value={formulaValue()}
            onInput={(e) => setFormulaValue(e.currentTarget.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") {
                commitCellEdit(e.currentTarget.value);
                setEditing(false);
                selectCell(activeCell().row + (e.shiftKey ? -1 : 1), activeCell().col);
                containerRef?.focus();
                e.preventDefault();
              } else if (e.key === "Escape") {
                setEditing(false);
                setFormulaValue(cellsData()[`${activeCell().row}:${activeCell().col}`]?.raw || "");
                containerRef?.focus();
                e.preventDefault();
              } else if (e.key === "Tab") {
                commitCellEdit(e.currentTarget.value);
                setEditing(false);
                selectCell(activeCell().row, activeCell().col + (e.shiftKey ? -1 : 1));
                containerRef?.focus();
                e.preventDefault();
              }
            }}
            ref={(el) => {
              setTimeout(() => {
                if (el) {
                  el.focus();
                  el.select();
                }
              }, 10);
            }}
            style={{
              position: "absolute",
              top: `${headerRowHeight - scrollTop() + offsetForRow(activeCell().row)}px`,
              left: `${headerColWidth - scrollLeft() + offsetForColumn(activeCell().col)}px`,
              width: `${widthAt(activeCell().col)}px`,
              height: `${heightAt(activeCell().row)}px`,
              border: "2px solid #16a34a",
              padding: "0 6px",
              margin: 0,
              "box-sizing": "border-box",
              "font-family": cellFont(activeStyle(), activeCell().row),
              "font-size": "12px",
              "background-color": "#ffffff",
              "z-index": 100,
              outline: "none",
            }}
          />
        </Show>
        </div>
        <IconSidebar panels={sidebarPanels()} defaultPanel="properties" />
      </div>

      <Show when={findBarOpen()}>
        <FindBar
          query={findQuery()}
          replaceWith={replaceWith()}
          showReplace={findReplaceMode()}
          matchCase={matchCase()}
          onQueryChange={setFindQuery}
          onReplaceChange={setReplaceWith}
          onFindNext={findNextCell}
          onReplace={() => void replaceCurrentCell()}
          onReplaceAll={() => void replaceAllCells()}
          onMatchCaseChange={setMatchCase}
          onClose={() => setFindBarOpen(false)}
        />
      </Show>

      {/* Sheet Tabs */}
      <footer class="g-sheet-tabs g-no-print">
        <For each={sheets()}>
          {(sheet, index) => {
            const active = () => index() === activeSheetIndex();
            return (
              <button
                type="button"
                onClick={() => {
                  const workbook = makeWorkbook(cellsData());
                  props.onChange?.(workbook);
                  setActiveSheetIndex(index());
                  const source = workbook.sheets?.[index()];
                  if (source) {
                    setFreezeRows(source.freezeRows || 0);
                    setFreezeCols(source.freezeCols || 0);
                    setColumnWidth(source.colWidths?.[0] || 100);
                    setRowHeight(source.rowHeights?.[0] || 26);
                    setColumnWidths(source.colWidths || {});
                    setRowHeights(source.rowHeights || {});
                    loadSheet(source);
                  }
                  emitCellInfo(activeCell().row, activeCell().col);
                  drawGrid();
                }}
                onDblClick={() => renameSheet(index())}
                onContextMenu={(e) => {
                  e.preventDefault();
                  const items: ContextMenuItem[] = [
                    { id: "rename-sheet", label: "Rename Sheet", action: () => renameSheet(index()) },
                    { id: "duplicate-sheet", label: "Duplicate Sheet", action: () => duplicateSheet(index()) },
                    { id: "delete-sheet", label: "Delete Sheet", action: () => deleteSheet(index()), disabled: sheets().length <= 1 },
                  ];
                  setContextMenu({ x: e.clientX, y: e.clientY, items });
                }}
                style={{
                  padding: "2px 10px",
                  height: "22px",
                  "border-radius": "0",
                  background: active() ? "var(--bg-surface)" : "transparent",
                  border: active() ? "1px solid var(--border-color)" : "1px solid transparent",
                  "border-bottom": active() ? "1px solid var(--bg-surface)" : "1px solid transparent",
                  "font-size": "11px",
                  "font-weight": active() ? "600" : "400",
                  color: active() ? "var(--text-primary)" : "var(--text-secondary)",
                }}
              >
                {sheet.name}
              </button>
            );
          }}
        </For>
        <ToolbarButton title="Add sheet" onClick={addSheet}><IconPlus /></ToolbarButton>
      </footer>

      <Show when={filterDropdownCol() !== null}>
        <div
          role="dialog"
          aria-label={`Filter column ${getColName(filterDropdownCol()!)}`}
          style={{
            position: "absolute",
            top: "96px",
            left: "56px",
            "z-index": 40,
            background: "var(--bg-surface)",
            border: "1px solid var(--border-color)",
            "border-radius": "6px",
            "box-shadow": "var(--shadow-md)",
            padding: "8px",
            width: "220px",
            "max-height": "280px",
            overflow: "auto",
          }}
        >
          <div style={{ display: "flex", gap: "6px", "margin-bottom": "8px" }}>
            <button type="button" class="g-toolbar-btn" onClick={() => setColumnFilterAll(filterDropdownCol()!, true)}>Select All</button>
            <button type="button" class="g-toolbar-btn" onClick={() => setColumnFilterAll(filterDropdownCol()!, false)}>Clear</button>
          </div>
          <div style={{ display: "flex", gap: "6px", "margin-bottom": "8px" }}>
            <button
              type="button"
              class="g-toolbar-btn"
              onClick={() => {
                setActiveCell({ row: filterBounds()?.startRow || 1, col: filterDropdownCol()! });
                void sortByActiveColumn(true);
              }}
            >
              Sort Asc
            </button>
            <button
              type="button"
              class="g-toolbar-btn"
              onClick={() => {
                setActiveCell({ row: filterBounds()?.startRow || 1, col: filterDropdownCol()! });
                void sortByActiveColumn(false);
              }}
            >
              Sort Desc
            </button>
          </div>
          <For each={uniqueColumnValues(filterDropdownCol()!)}>
            {(value) => {
              const col = () => filterDropdownCol()!;
              const all = () => uniqueColumnValues(col());
              const allowed = () => columnFilters()[col()] ?? all();
              const checked = () => allowed().includes(value);
              return (
                <label style={{ display: "flex", gap: "8px", "align-items": "center", padding: "2px 0", "font-size": "12px" }}>
                  <input
                    type="checkbox"
                    checked={checked()}
                    onChange={() => toggleFilterValue(col(), value)}
                  />
                  <span>{value || "(blank)"}</span>
                </label>
              );
            }}
          </For>
          <button type="button" class="g-toolbar-btn" style={{ "margin-top": "8px", width: "100%" }} onClick={() => setFilterDropdownCol(null)}>
            Close
          </button>
        </div>
      </Show>

      <SortDialog
        open={sortDialogOpen()}
        sortKeys={sortKeys()}
        onSortKeysChange={setSortKeys}
        getColName={getColName}
        parseColLetter={parseColLetter}
        activeCol={activeCell().col}
        onClose={() => setSortDialogOpen(false)}
        onApply={() => void applyMultiSort()}
      />

      <PrintDialog
        open={printDialogOpen()}
        printMode={printMode()}
        printFitWidth={printFitWidth()}
        onPrintModeChange={setPrintMode}
        onPrintFitWidthChange={setPrintFitWidth}
        onClose={() => setPrintDialogOpen(false)}
        onPrint={printSheet}
      />

      <NumberFormatDialog
        open={numberFormatOpen()}
        onClose={() => setNumberFormatOpen(false)}
        value={activeNumberFormat()}
        onApply={applyNumberFormat}
      />

      <DataValidationDialog
        open={validationOpen()}
        onClose={() => setValidationOpen(false)}
        value={activeValidation()}
        onApply={applyValidation}
      />

      <Show when={contextMenu()}>
        {(menu) => (
          <ContextMenu
            x={menu().x}
            y={menu().y}
            items={menu().items}
            onClose={() => setContextMenu(null)}
          />
        )}
      </Show>
    </div>
  );
}
