import { describe, expect, it } from "vitest";
import { wrapText } from "./slidePngExport";

const measureFixed = (charWidth: number) => (text: string) => ({ width: text.length * charWidth });

describe("slidePngExport wrapText", () => {
  it("breaks text into lines that fit the maximum width", () => {
    const ctx = { measureText: measureFixed(1) };
    const lines = wrapText(ctx, "aa bb cc dd", 5);
    expect(lines.every((line) => line.length <= 5)).toBe(true);
    expect(lines.join(" ")).toBe("aa bb cc dd");
  });

  it("keeps single long words unclipped rather than dropping them", () => {
    const ctx = { measureText: measureFixed(1) };
    expect(wrapText(ctx, "supercalifragilistic", 5)).toEqual(["supercalifragilistic"]);
  });

  it("preserves explicit newlines as separate lines", () => {
    const ctx = { measureText: measureFixed(1) };
    expect(wrapText(ctx, "ab\ncd", 10)).toEqual(["ab", "cd"]);
  });
});
