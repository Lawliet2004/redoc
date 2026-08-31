import { describe, expect, it } from "vitest";
import { conditionalRuleMatches, conditionalStyleForCell } from "./conditionalFormatting";
import type { GridCell } from "./sheetTypes";

const cell = (raw: string): GridCell => ({ raw, display: raw });
const range = { startRow: 1, endRow: 4, startCol: 1, endCol: 3 };

describe("conditional formatting", () => {
  it("matches numeric and text rules only inside their range", () => {
    expect(conditionalRuleMatches(cell("12"), 2, 2, { range, type: "greaterThan", value: "10" })).toBe(true);
    expect(conditionalRuleMatches(cell("8"), 2, 2, { range, type: "greaterThan", value: "10" })).toBe(false);
    expect(conditionalRuleMatches(cell("ready"), 2, 2, { range, type: "textContains", value: "ead" })).toBe(true);
    expect(conditionalRuleMatches(cell("12"), 8, 2, { range, type: "greaterThan", value: "10" })).toBe(false);
  });

  it("returns the first matching style without mutating source cell", () => {
    const rule = { range, type: "equalTo" as const, value: "12", style: { bgColor: "#dcfce7", bold: true } };
    const source = cell("12");
    expect(conditionalStyleForCell(source, 1, 1, [rule])).toEqual(rule.style);
    expect(source.style).toBeUndefined();
  });

  it("maps color-scale values into stable buckets", () => {
    const rule = {
      range,
      type: "colorScale" as const,
      scaleColors: ["#fee2e2", "#fef08a", "#dcfce7"],
    };
    expect(conditionalStyleForCell(cell("0"), 1, 1, [rule])?.bgColor).toBe("#fee2e2");
    expect(conditionalStyleForCell(cell("50"), 1, 1, [rule])?.bgColor).toBe("#fef08a");
    expect(conditionalStyleForCell(cell("100"), 1, 1, [rule])?.bgColor).toBe("#dcfce7");
  });
});
