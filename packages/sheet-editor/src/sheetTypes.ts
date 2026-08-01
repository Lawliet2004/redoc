import type { Accessor, Setter } from "solid-js";
import type { ListValidation } from "./DataValidationDialog";

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
  validation?: ListValidation;
  wrap?: boolean;
  fontFamily?: string;
  fontSize?: number;
};

export type GridCell = {
  raw: string;
  display: string;
  style?: GridCellStyle;
};

export type MergeRange = { startRow: number; endRow: number; startCol: number; endCol: number };

export type SheetSnapshot = {
  cells: Record<string, GridCell>;
  merges: MergeRange[];
  autoFilterEnabled: boolean;
  filterRange: { startRow: number; endRow: number; startCol: number; endCol: number } | null;
  columnFilters: Record<number, string[]>;
  columnWidths: Record<number, number>;
  rowHeights: Record<number, number>;
  chartType: "bar" | "line" | "pie" | null;
  chartTitle: string;
  chartRange: { startRow: number; endRow: number; startCol: number; endCol: number };
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
  chartType: Accessor<"bar" | "line" | "pie" | null>;
  chartData: () => Array<{ label: string; value: number }>;
  chartMax: () => number;
  pieSlices: () => Array<{
    isFullCircle: boolean;
    pathData: string;
    color: string;
    label: string;
    value: number;
  }>;
}
