import type { GridCell, MergeRange } from "./sheetTypes";

export type PivotAggregation = "sum" | "count" | "average";

export interface PivotTableConfig {
  id: string;
  sourceRange: MergeRange;
  rowField: number;
  valueField: number;
  aggregation: PivotAggregation;
  outputStartRow: number;
  outputStartCol: number;
  outputRowCount?: number;
}

export interface PivotTableResult {
  cells: Record<string, GridCell>;
  rowCount: number;
  warning?: string;
}

const textFor = (cells: Record<string, GridCell>, row: number, col: number) => {
  const cell = cells[`${row}:${col}`];
  return String(cell?.display ?? cell?.raw ?? "").trim();
};

/** Build a deterministic one-row-field pivot summary from a rectangular source. */
export function buildPivotTable(cells: Record<string, GridCell>, config: PivotTableConfig): PivotTableResult {
  const source = config.sourceRange;
  if (source.startRow >= source.endRow || source.startCol > source.endCol) {
    return { cells: {}, rowCount: 0, warning: "Pivot source must include a header and at least one data row." };
  }
  if (
    config.rowField < source.startCol || config.rowField > source.endCol ||
    config.valueField < source.startCol || config.valueField > source.endCol
  ) {
    return { cells: {}, rowCount: 0, warning: "Pivot fields must be inside the source range." };
  }

  const groups = new Map<string, { sum: number; count: number }>();
  let grandSum = 0;
  let grandCount = 0;
  for (let row = source.startRow + 1; row <= source.endRow; row += 1) {
    const key = textFor(cells, row, config.rowField) || "(blank)";
    const raw = textFor(cells, row, config.valueField);
    const value = Number(raw.replace(/,/g, ""));
    const current = groups.get(key) || { sum: 0, count: 0 };
    if (config.aggregation === "count") {
      if (raw) {
        current.count += 1;
        grandCount += 1;
      }
    } else if (Number.isFinite(value)) {
      current.sum += value;
      current.count += 1;
      grandSum += value;
      grandCount += 1;
    }
    groups.set(key, current);
  }

  const rowHeader = textFor(cells, source.startRow, config.rowField) || "Row Labels";
  const valueHeader = textFor(cells, source.startRow, config.valueField) || "Values";
  const headerValue = `${config.aggregation === "count" ? "Count" : config.aggregation === "average" ? "Average" : "Sum"} of ${valueHeader}`;
  const rows: string[][] = [[rowHeader, headerValue]];
  for (const [key, aggregate] of groups) {
    const value = config.aggregation === "count"
      ? aggregate.count
      : config.aggregation === "average"
        ? (aggregate.count ? aggregate.sum / aggregate.count : 0)
        : aggregate.sum;
    const formatted = config.aggregation === "average"
      ? value.toFixed(2)
      : Number.isInteger(value)
        ? String(value)
        : value.toFixed(2);
    rows.push([key, formatted]);
  }
  const grandTotal = config.aggregation === "count"
    ? grandCount
    : config.aggregation === "average"
      ? (grandCount ? grandSum / grandCount : 0)
      : grandSum;
  rows.push(["Grand Total", config.aggregation === "average" ? grandTotal.toFixed(2) : String(grandTotal)]);

  const output: Record<string, GridCell> = {};
  rows.forEach((row, rowOffset) => row.forEach((value, colOffset) => {
    output[`${config.outputStartRow + rowOffset}:${config.outputStartCol + colOffset}`] = {
      raw: value,
      display: value,
      style: rowOffset === 0 || row[0] === "Grand Total"
        ? { bold: true, bgColor: "#e2e8f0" }
        : undefined,
    };
  }));
  return { cells: output, rowCount: rows.length };
}
