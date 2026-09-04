import type { ConditionalFormattingRule, GridCell, GridCellStyle } from "./sheetTypes";

function numericValue(cell: GridCell): number | null {
  const value = Number(cell.raw);
  return Number.isFinite(value) && cell.raw.trim() !== "" ? value : null;
}

function scalePercent(value: number, rule: ConditionalFormattingRule): number {
  const configuredMin = Number(rule.value);
  const configuredMax = Number(rule.value2);
  const min = Number.isFinite(configuredMin) ? configuredMin : 0;
  const max = Number.isFinite(configuredMax) ? configuredMax : 100;
  if (max === min) return value >= max ? 1 : 0;
  const low = Math.min(min, max);
  const high = Math.max(min, max);
  return Math.max(0, Math.min(1, (value - low) / (high - low)));
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
    const number = numericValue(cell);
    if (rule.type === "colorScale" && rule.scaleColors?.length) {
      // Color position is relative to the rule's configured min/max domain,
      // falling back to 0-100 like the dataBar path — never a raw /100
      // regardless of the values actually present.
      const normalized = scalePercent(number ?? 0, rule);
      const index = Math.min(
        rule.scaleColors.length - 1,
        Math.max(0, Math.round(normalized * (rule.scaleColors.length - 1))),
      );
      return { ...rule.style, bgColor: rule.scaleColors[index] };
    }
    if (rule.type === "dataBar" && number !== null) {
      return {
        ...rule.style,
        conditionalBarPercent: scalePercent(number, rule),
        conditionalBarColor: rule.style?.bgColor || "#60a5fa",
      };
    }
    return rule.style;
  }
  return undefined;
}
