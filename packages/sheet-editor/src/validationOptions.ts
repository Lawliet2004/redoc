import type { ListValidation } from "./DataValidationDialog";
import type { GridCell } from "./sheetTypes";

export type ValidationNamedRange = {
  name: string;
  rangeStr: string;
  sheet: string | null;
};

type CellRange = {
  sheet: string | null;
  startRow: number;
  startCol: number;
  endRow: number;
  endCol: number;
};

const MAX_SOURCE_CELLS = 10_000;
const MAX_OPTIONS = 512;

function columnNumber(value: string): number | null {
  let result = 0;
  for (const char of value.toUpperCase()) {
    const code = char.charCodeAt(0);
    if (code < 65 || code > 90) return null;
    result = result * 26 + code - 64;
    if (result > 100_000) return null;
  }
  return result || null;
}

function parseCell(value: string): { row: number; col: number } | null {
  const match = value.trim().match(/^\$?([A-Z]+)\$?(\d+)$/i);
  if (!match) return null;
  const col = columnNumber(match[1]);
  const row = Number(match[2]);
  if (!col || !Number.isSafeInteger(row) || row < 1 || row > 100_000) return null;
  return { row, col };
}

function parseRange(value: string): CellRange | null {
  const trimmed = value.trim().replace(/^=/, "").trim();
  const bang = trimmed.lastIndexOf("!");
  let sheet: string | null = null;
  let rangeText = trimmed;
  if (bang >= 0) {
    sheet = trimmed.slice(0, bang).trim();
    if (sheet.startsWith("'") && sheet.endsWith("'")) sheet = sheet.slice(1, -1).replace(/''/g, "'");
    rangeText = trimmed.slice(bang + 1);
  }
  const parts = rangeText.split(":");
  if (parts.length > 2) return null;
  const start = parseCell(parts[0]);
  const end = parseCell(parts[1] ?? parts[0]);
  if (!start || !end) return null;
  const startRow = Math.min(start.row, end.row);
  const endRow = Math.max(start.row, end.row);
  const startCol = Math.min(start.col, end.col);
  const endCol = Math.max(start.col, end.col);
  if ((endRow - startRow + 1) * (endCol - startCol + 1) > MAX_SOURCE_CELLS) return null;
  return { sheet, startRow, endRow, startCol, endCol };
}

function resolveRange(
  formula: string,
  namedRanges: ValidationNamedRange[],
  activeSheetName: string,
  depth = 0,
): CellRange | null {
  if (depth > 2) return null;
  const source = formula.trim().replace(/^=/, "").trim();
  const named = namedRanges.find((entry) => entry.name.toLowerCase() === source.toLowerCase());
  if (named) {
    const resolved = resolveRange(named.rangeStr, namedRanges, activeSheetName, depth + 1);
    return resolved ? { ...resolved, sheet: named.sheet ?? resolved.sheet } : null;
  }
  return parseRange(source);
}

export function resolveValidationOptions(
  validation: ListValidation | undefined,
  cells: Record<string, GridCell>,
  namedRanges: ValidationNamedRange[] = [],
  activeSheetName = "",
  cellsBySheet: Record<string, Record<string, GridCell>> = {},
): string[] {
  if (!validation || validation.type !== "list") return [];
  if (validation.options.length) return [...new Set(validation.options)].slice(0, MAX_OPTIONS);
  if (!validation.formula) return [];
  const range = resolveRange(validation.formula, namedRanges, activeSheetName);
  if (!range) return [];
  let sourceCells = cells;
  if (range.sheet && (!activeSheetName || range.sheet.toLowerCase() !== activeSheetName.toLowerCase())) {
    const source = Object.entries(cellsBySheet).find(([name]) => name.toLowerCase() === range.sheet!.toLowerCase());
    if (!source) return [];
    sourceCells = source[1];
  }
  const values: string[] = [];
  const seen = new Set<string>();
  for (let row = range.startRow; row <= range.endRow && values.length < MAX_OPTIONS; row += 1) {
    for (let col = range.startCol; col <= range.endCol && values.length < MAX_OPTIONS; col += 1) {
      const cell = sourceCells[`${row}:${col}`];
      const value = (cell?.display || cell?.raw || "").trim();
      if (value && !seen.has(value)) {
        seen.add(value);
        values.push(value);
      }
    }
  }
  return values;
}
