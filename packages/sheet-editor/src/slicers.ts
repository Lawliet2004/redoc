import type { GridCell, MergeRange, SlicerConfig } from "./sheetTypes";

export const MAX_SLICER_VALUES = 200;

const cellText = (cells: Record<string, GridCell>, row: number, col: number) =>
  String(cells[`${row}:${col}`]?.display ?? cells[`${row}:${col}`]?.raw ?? "").trim();

export function slicerValues(
  cells: Record<string, GridCell>,
  slicer: Pick<SlicerConfig, "sourceRange" | "column">,
): string[] {
  const values = new Set<string>();
  for (let row = slicer.sourceRange.startRow + 1; row <= slicer.sourceRange.endRow; row += 1) {
    values.add(cellText(cells, row, slicer.column));
  }
  return Array.from(values)
    .sort((left, right) => left.localeCompare(right))
    .slice(0, MAX_SLICER_VALUES);
}

export function createSlicer(
  cells: Record<string, GridCell>,
  sourceRange: MergeRange,
  column: number,
  title: string,
  id: string,
): SlicerConfig {
  if (sourceRange.startRow >= sourceRange.endRow) {
    throw new Error("Slicer source must include a header and at least one data row.");
  }
  if (
    sourceRange.startCol > sourceRange.endCol ||
    column < sourceRange.startCol ||
    column > sourceRange.endCol
  ) {
    throw new Error("Slicer column must be inside the source range.");
  }
  const header = cellText(cells, sourceRange.startRow, column);
  return {
    id,
    title: title.trim() || header || `Column ${column}`,
    sourceRange: { ...sourceRange },
    column,
    selectedValues: [],
  };
}

export function toggleSlicerValue(
  slicer: SlicerConfig,
  value: string,
  availableValues: string[],
): SlicerConfig {
  const available = new Set(availableValues);
  const selected = slicer.selectedValues.length
    ? new Set(slicer.selectedValues.filter((entry) => available.has(entry)))
    : new Set(availableValues);
  if (selected.has(value)) selected.delete(value);
  else selected.add(value);
  const nextValues = selected.size === available.size
    ? []
    : Array.from(selected).sort((left, right) => left.localeCompare(right));
  return { ...slicer, selectedValues: nextValues };
}

export function rowMatchesSlicers(
  cells: Record<string, GridCell>,
  row: number,
  slicers: SlicerConfig[],
): boolean {
  return slicers.every((slicer) => {
    const range = slicer.sourceRange;
    if (row <= range.startRow || row > range.endRow) return true;
    if (slicer.selectedValues.length === 0) return true;
    return slicer.selectedValues.includes(cellText(cells, row, slicer.column));
  });
}
