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

  it("scales color-scale against the configured min/max domain, not raw /100", () => {
    const rule = {
      range,
      type: "colorScale" as const,
      value: "1000",
      value2: "2000",
      scaleColors: ["#fee2e2", "#fef08a", "#dcfce7"],
    };
    // 1500 is the midpoint of 1000..2000 — must map to the middle bucket,
    // which the old value/100 heuristic (1500/100 clamped to 1) got wrong.
    expect(conditionalStyleForCell(cell("1500"), 1, 1, [rule])?.bgColor).toBe("#fef08a");
    // 1200 → 20% of the domain → lowest bucket.
    expect(conditionalStyleForCell(cell("1200"), 1, 1, [rule])?.bgColor).toBe("#fee2e2");
    // 2000 → top of the domain → highest bucket.
    expect(conditionalStyleForCell(cell("2000"), 1, 1, [rule])?.bgColor).toBe("#dcfce7");
  });

  it("returns bounded data-bar render metadata", () => {
    const rule = {
      range,
      type: "dataBar" as const,
      style: { bgColor: "#2563eb" },
    };
    expect(conditionalStyleForCell(cell("40"), 1, 1, [rule])).toMatchObject({
      conditionalBarPercent: 0.4,
      conditionalBarColor: "#2563eb",
    });
    expect(conditionalStyleForCell(cell("250"), 1, 1, [rule])?.conditionalBarPercent).toBe(1);
    expect(conditionalStyleForCell(cell("-5"), 1, 1, [rule])?.conditionalBarPercent).toBe(0);
    expect(conditionalStyleForCell(cell("30"), 1, 1, [{ ...rule, value: "10", value2: "50" }])?.conditionalBarPercent).toBe(0.5);
  });
});
