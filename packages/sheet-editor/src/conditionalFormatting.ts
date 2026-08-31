import type { ConditionalFormattingRule, GridCell, GridCellStyle } from "./sheetTypes";

function numericValue(cell: GridCell): number | null {
  const value = Number(cell.raw);
  return Number.isFinite(value) && cell.raw.trim() !== "" ? value : null;
}

export function conditionalRuleMatches(
  cell: GridCell,
  row: number,
  col: number,
  rule: ConditionalFormattingRule,
): boolean {
  const { range } = rule;
  if (row < range.startRow || row > range.endRow || col < range.startCol || col > range.endCol) return false;
  const raw = cell.raw;
  const number = numericValue(cell);
  switch (rule.type) {
    case "greaterThan":
      return number !== null && number > Number(rule.value);
    case "lessThan":
      return number !== null && number < Number(rule.value);
    case "equalTo":
      return number !== null ? number === Number(rule.value) : raw === (rule.value || "");
    case "textContains":
      return raw.toLocaleLowerCase().includes((rule.value || "").toLocaleLowerCase());
    case "dataBar":
    case "colorScale":
      return number !== null;
    default:
      return false;
  }
}

export function conditionalStyleForCell(
  cell: GridCell,
  row: number,
  col: number,
  rules: readonly ConditionalFormattingRule[],
): GridCellStyle | undefined {
  for (const rule of rules) {
    if (!conditionalRuleMatches(cell, row, col, rule)) continue;
    if (rule.type === "colorScale" && rule.scaleColors?.length) {
      const index = Math.min(rule.scaleColors.length - 1, Math.max(0, Math.floor(Number(cell.raw) % rule.scaleColors.length)));
      return { ...rule.style, bgColor: rule.scaleColors[index] };
    }
    return rule.style;
  }
  return undefined;
}
