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

/** Multi-field pivot: rows × columns with one aggregated value. */
export interface MultiFieldPivotConfig {
  sourceRange: MergeRange;
  rowField: number;
  columnField: number;
  valueField: number;
  aggregation: PivotAggregation;
  outputStartRow: number;
  outputStartCol: number;
}

export interface MultiFieldPivotResult {
  cells: Record<string, GridCell>;
  rowCount: number;
  colCount: number;
  warning?: string;
}

const textFor = (cells: Record<string, GridCell>, row: number, col: number) => {
  const cell = cells[`${row}:${col}`];
  return String(cell?.display ?? cell?.raw ?? "").trim();
};

/** Build a deterministic one-row-field pivot summary from a rectangular source. */
export function buildPivotTable(
  cells: Record<string, GridCell>,
  config: PivotTableConfig,
  rowVisible: (row: number) => boolean = () => true,
): PivotTableResult {
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
    if (!rowVisible(row)) continue;
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

const AGGREGATE_LABEL: Record<PivotAggregation, string> = {
  sum: "Sum",
  count: "Count",
  average: "Average",
};

function formatAggregate(aggregation: PivotAggregation, sum: number, count: number): string {
  const value =
    aggregation === "count"
      ? count
      : aggregation === "average"
        ? count ? sum / count : 0
        : sum;
  return aggregation === "average"
    ? value.toFixed(2)
    : Number.isInteger(value)
      ? String(value)
      : value.toFixed(2);
}

const MAX_PIVOT_GROUPS = 500;

/**
 * Build a rows × columns pivot with one aggregated value field.
 * Groups are insertion-ordered (deterministic), capped, and each axis gets
 * a Grand Total. Mirrors Excel's basic PivotTable shape without the
 * interactive field list.
 */
export function buildMultiFieldPivotTable(
  cells: Record<string, GridCell>,
  config: MultiFieldPivotConfig,
  rowVisible: (row: number) => boolean = () => true,
): MultiFieldPivotResult {
  const source = config.sourceRange;
  if (source.startRow >= source.endRow || source.startCol > source.endCol) {
    return { cells: {}, rowCount: 0, colCount: 0, warning: "Pivot source must include a header and at least one data row." };
  }
  for (const field of [config.rowField, config.columnField, config.valueField]) {
    if (field < source.startCol || field > source.endCol) {
      return { cells: {}, rowCount: 0, colCount: 0, warning: "Pivot fields must be inside the source range." };
    }
  }
  if (config.rowField === config.columnField) {
    return { cells: {}, rowCount: 0, colCount: 0, warning: "Row and column fields must differ." };
  }

  // (rowKey, columnKey) -> { sum, count }
  const aggregates = new Map<string, { sum: number; count: number }>();
  const rowKeys: string[] = [];
  const columnKeys: string[] = [];
  const seenRows = new Set<string>();
  const seenColumns = new Set<string>();
  for (let row = source.startRow + 1; row <= source.endRow; row += 1) {
    if (!rowVisible(row)) continue;
    const rowKey = textFor(cells, row, config.rowField) || "(blank)";
    const columnKey = textFor(cells, row, config.columnField) || "(blank)";
    if (!seenRows.has(rowKey)) {
      if (seenRows.size >= MAX_PIVOT_GROUPS) {
        return { cells: {}, rowCount: 0, colCount: 0, warning: `Pivot exceeds the ${MAX_PIVOT_GROUPS}-group bound.` };
      }
      seenRows.add(rowKey);
      rowKeys.push(rowKey);
    }
    if (!seenColumns.has(columnKey)) {
      if (seenColumns.size >= MAX_PIVOT_GROUPS) {
        return { cells: {}, rowCount: 0, colCount: 0, warning: `Pivot exceeds the ${MAX_PIVOT_GROUPS}-group bound.` };
      }
      seenColumns.add(columnKey);
      columnKeys.push(columnKey);
    }
    const raw = textFor(cells, row, config.valueField);
    const value = Number(raw.replace(/,/g, ""));
    const key = `${rowKey}\u0000${columnKey}`;
    const current = aggregates.get(key) || { sum: 0, count: 0 };
    if (config.aggregation === "count") {
      if (raw) current.count += 1;
    } else if (Number.isFinite(value)) {
      current.sum += value;
      current.count += 1;
    }
    aggregates.set(key, current);
  }

  const rowHeader = textFor(cells, source.startRow, config.rowField) || "Row Labels";
  const columnHeader = textFor(cells, source.startRow, config.columnField) || "Column Labels";
  const valueHeader = textFor(cells, source.startRow, config.valueField) || "Values";
  const aggLabel = `${AGGREGATE_LABEL[config.aggregation]} of ${valueHeader}`;

  // Grid: [header row: rowHeader | columnKeys...] then one row per rowKey,
  // ending with a Grand Total row and a Grand Total column.
  const grid: string[][] = [];
  const header = [rowHeader, ...columnKeys, "Grand Total"];
  grid.push(header);
  // Axis label carries the aggregation name so the summary is self-describing.
  grid[0][0] = `${aggLabel} · ${rowHeader} \\ ${columnHeader}`.slice(0, 60);

  const cellAggregate = (rowKey: string, columnKey: string) =>
    aggregates.get(`${rowKey}\u0000${columnKey}`) || { sum: 0, count: 0 };

  for (const rowKey of rowKeys) {
    const row: string[] = [rowKey];
    let rowSum = 0;
    let rowCount = 0;
    for (const columnKey of columnKeys) {
      const aggregate = cellAggregate(rowKey, columnKey);
      row.push(formatAggregate(config.aggregation, aggregate.sum, aggregate.count));
      rowSum += aggregate.sum;
      rowCount += aggregate.count;
    }
    row.push(formatAggregate(config.aggregation, rowSum, rowCount));
    grid.push(row);
  }
  const totalRow: string[] = ["Grand Total"];
  let grandSum = 0;
  let grandCount = 0;
  for (const columnKey of columnKeys) {
    let columnSum = 0;
    let columnCount = 0;
    for (const rowKey of rowKeys) {
      const aggregate = cellAggregate(rowKey, columnKey);
      columnSum += aggregate.sum;
      columnCount += aggregate.count;
    }
    totalRow.push(formatAggregate(config.aggregation, columnSum, columnCount));
    grandSum += columnSum;
    grandCount += columnCount;
  }
  totalRow.push(formatAggregate(config.aggregation, grandSum, grandCount));
  grid.push(totalRow);

  const output: Record<string, GridCell> = {};
  const isHeaderRow = (rowIndex: number) => rowIndex === 0;
  const isTotalRow = (rowIndex: number) => grid[rowIndex][0] === "Grand Total";
  const isTotalCol = (colIndex: number) => colIndex === grid[0].length - 1;
  grid.forEach((row, rowOffset) => row.forEach((value, colOffset) => {
    output[`${config.outputStartRow + rowOffset}:${config.outputStartCol + colOffset}`] = {
      raw: value,
      display: value,
      style: isHeaderRow(rowOffset) || isTotalRow(rowOffset) || isTotalCol(colOffset)
        ? { bold: true, bgColor: "#e2e8f0" }
        : undefined,
    };
  }));

  return {
    cells: output,
    rowCount: grid.length,
    colCount: grid[0].length,
  };
}
