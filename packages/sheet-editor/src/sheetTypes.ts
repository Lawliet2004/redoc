import type { Accessor, Setter } from "solid-js";
import type { ListValidation } from "./DataValidationDialog";

export type BorderLineStyle = "thin" | "medium" | "thick" | "dashed" | "dotted" | "double";

export type BorderEdge = {
  style: BorderLineStyle;
  color?: string;
};

export type CellBorders = {
  top?: BorderEdge;
  right?: BorderEdge;
  bottom?: BorderEdge;
  left?: BorderEdge;
};

export type GridCellStyle = {
  bold?: boolean;
  italic?: boolean;
  underline?: boolean;
  fontColor?: string;
  bgColor?: string;
  align?: "left" | "center" | "right";
  vAlign?: "top" | "middle" | "bottom";
  format?: "general" | "currency" | "percent" | "number" | "date" | "text";
  decimals?: number;
  hyperlink?: string;
  /** Bounded inline data URI shown inside the cell in Redoc. */
  image?: string;
  validation?: ListValidation;
  wrap?: boolean;
  fontFamily?: string;
  fontSize?: number;
  /** Per-side cell borders; round-trips through native XLSX. */
  borders?: CellBorders;
  /** Ephemeral render metadata added by conditional formatting. */
  conditionalBarPercent?: number;
  conditionalBarColor?: string;
};

export type GridCell = {
  raw: string;
  display: string;
  style?: GridCellStyle;
  /** Render-only value produced by a dynamic-array formula. */
  spill?: boolean;
};

export type MergeRange = { startRow: number; endRow: number; startCol: number; endCol: number };

export type ConditionalFormattingRule = {
  range: { startRow: number; endRow: number; startCol: number; endCol: number };
  type: "greaterThan" | "lessThan" | "equalTo" | "textContains" | "dataBar" | "colorScale";
  value?: string;
  value2?: string;
  style?: { fontColor?: string; bgColor?: string; bold?: boolean; italic?: boolean };
  scaleColors?: string[];
};

export type PivotAggregation = "sum" | "count" | "average";

export type PivotTableConfig = {
  id: string;
  sourceRange: MergeRange;
  rowField: number;
  /** When set, the pivot crosses rows × this column field (multi-field). */
  columnField?: number;
  valueField: number;
  aggregation: PivotAggregation;
  outputStartRow: number;
  outputStartCol: number;
  outputRowCount?: number;
  outputColCount?: number;
};

export type ScenarioCellChange = { row: number; col: number; rawValue: string };

export type ScenarioConfig = {
  id: string;
  name: string;
  changes: ScenarioCellChange[];
};

export type SlicerConfig = {
  id: string;
  title: string;
  sourceRange: MergeRange;
  column: number;
  selectedValues: string[];
};

/** Cell-anchored review comment (offline collaboration). */
export type CellComment = {
  id: string;
  row: number;
  col: number;
  author: string;
  text: string;
  resolved: boolean;
  createdAt: string;
};

export type SheetSnapshot = {
  cells: Record<string, GridCell>;
  merges: MergeRange[];
  autoFilterEnabled: boolean;
  filterRange: { startRow: number; endRow: number; startCol: number; endCol: number } | null;
  columnFilters: Record<number, string[]>;
  columnWidths: Record<number, number>;
  rowHeights: Record<number, number>;
  chartType: "bar" | "line" | "pie" | "area" | "scatter" | "doughnut" | null;
  chartTitle: string;
  chartRange: { startRow: number; endRow: number; startCol: number; endCol: number };
  conditionalFormatting: ConditionalFormattingRule[];
  pivotTables?: PivotTableConfig[];
  scenarios?: ScenarioConfig[];
  slicers?: SlicerConfig[];
  comments?: CellComment[];
  sheetsMeta: Array<{ id: string; name: string }>;
  activeSheetIndex: number;
};

export interface FormulaBarProps {
  activeCell: Accessor<{ row: number; col: number }>;
  formulaValue: Accessor<string>;
  setFormulaValue: Setter<string>;
  getColName: (col: number) => string;
  insertFormulaPrefix: (text: string) => void;
  commitCellEdit: (value: string) => void | Promise<void>;
  editing: Accessor<boolean>;
  setEditing: Setter<boolean>;
  cellsData: Accessor<Record<string, GridCell>>;
  cellsBySheet?: Accessor<Record<string, Record<string, GridCell>>>;
  namedRanges?: Accessor<Array<{ name: string; rangeStr: string; sheet: string | null }>>;
  activeSheetName?: Accessor<string>;
  containerRef?: HTMLDivElement;
  formulaInputRef?: HTMLInputElement;
}

export interface SheetToolbarProps {
  fontFamily: Accessor<string>;
  setFontFamily: Setter<string>;
  fontSize: Accessor<string>;
  setFontSize: Setter<string>;
  activeStyle: () => GridCellStyle | undefined;
  updateActiveStyle: (changes: GridCellStyle) => void;
  selectedBounds: () => { startRow: number; endRow: number; startCol: number; endCol: number };
  activeCell: Accessor<{ row: number; col: number }>;
  setMerges: Setter<MergeRange[]>;
  pushHistory: () => void;
  makeWorkbook: (cellMap: Record<string, GridCell>) => any;
  cellsData: Accessor<Record<string, GridCell>>;
  drawGrid: () => void;
  onChange?: (json: any) => void;
  onOpenNumberFormat?: () => void;
  onOpenValidation?: () => void;
  /** Applies a border preset + line style + optional color across the selection. */
  onApplyBorders?: (preset: "all" | "outer" | "top" | "bottom" | "none", style: string, color?: string) => void;
}

export interface GridCanvasProps {
  containerRef: HTMLDivElement;
  canvasRef: HTMLCanvasElement;
  scrollTop: Accessor<number>;
  scrollLeft: Accessor<number>;
  setScrollTop: Setter<number>;
  setScrollLeft: Setter<number>;
  rowHeight: Accessor<number>;
  columnWidth: Accessor<number>;
  drawGrid: () => void;
  markGridDirtyFull?: () => void;
  activeCell: Accessor<{ row: number; col: number }>;
  redo: () => void | Promise<void>;
  undo: () => void | Promise<void>;
  copySelection: () => void | Promise<void>;
  cutSelection: () => void | Promise<void>;
  pasteValuesOnly: () => void | Promise<void>;
  pasteTsv: () => void | Promise<void>;
  selectCell: (row: number, col: number, extend?: boolean) => void;
  moveActiveCell: (dRow: number, dCol: number, extend?: boolean) => void;
  lastUsedCell: () => { row: number; col: number };
  jumpToDataEdge: (dRow: number, dCol: number, extend?: boolean) => void;
  setEditing: Setter<boolean>;
  setFormulaValue: Setter<string>;
  formulaInputRef?: HTMLInputElement;
  handleCanvasClick: (event: MouseEvent) => void;
  setContextMenu: Setter<{ x: number; y: number; items: import("@redoc/editor-common").ContextMenuItem[] } | null>;
  emitEditorCommand: typeof import("@redoc/editor-common").emitEditorCommand;
  isFillHandlePoint: (event: MouseEvent) => boolean;
  setFillDragStart: Setter<{ row: number; col: number } | null>;
  setSuppressNextClick: Setter<boolean>;
  startDimensionDrag: (event: PointerEvent) => boolean;
  updateDimensionDrag: (event: PointerEvent) => void;
  fillDragStart: Accessor<{ row: number; col: number } | null>;
  finishFillDrag: (event: MouseEvent) => void | Promise<void>;
  dimensionDrag: Accessor<{
    axis: "column" | "row";
    index: number;
    pointer: number;
    size: number;
  } | null>;
  setDimensionDrag: Setter<{
    axis: "column" | "row";
    index: number;
    pointer: number;
    size: number;
  } | null>;
  getColName: (col: number) => string;
  cellsData: Accessor<Record<string, GridCell>>;
  cellHyperlink: (cell: GridCell | undefined) => string | null;
  onOpenHyperlink?: (href: string) => boolean;
  chartType: Accessor<"bar" | "line" | "pie" | "area" | "scatter" | "doughnut" | null>;
  chartTitle?: string;
  chartTop: () => number;
  chartLeft: () => number;
  chartData: () => Array<{ label: string; value: number }>;
  chartMax: () => number;
  pieSlices: () => Array<{
    isFullCircle: boolean;
    pathData: string;
    color: string;
    label: string;
    value: number;
  }>;
  conditionalFormatting: Accessor<ConditionalFormattingRule[]>;
}
