import { describe, expect, it } from "vitest";
import {
  applyBorderPresetToStyle,
  bordersForPresetCell,
  borderEdgeWidth,
  normalizeBorderLineStyle,
  rangePosition,
} from "./cellBorders";

const pos = (over: Partial<ReturnType<typeof rangePosition>> = {}) => ({
  isFirstRow: true,
  isLastRow: true,
  isFirstCol: true,
  isLastCol: true,
  ...over,
});

describe("cellBorders", () => {
  it("accepts only the bounded line-style vocabulary", () => {
    expect(normalizeBorderLineStyle("thin")).toBe("thin");
    expect(normalizeBorderLineStyle("double")).toBe("double");
    expect(normalizeBorderLineStyle("wavy")).toBeNull();
    expect(normalizeBorderLineStyle(null)).toBeNull();
  });

  it("maps line styles to canvas widths", () => {
    expect(borderEdgeWidth("thin")).toBe(1);
    expect(borderEdgeWidth("medium")).toBe(2);
    expect(borderEdgeWidth("thick")).toBe(3);
  });

  it("outer preset boxes a single cell", () => {
    const borders = bordersForPresetCell("outer", { style: "medium", color: "#123456" }, pos());
    expect(borders?.top?.style).toBe("medium");
    expect(borders?.top?.color).toBe("#123456");
    expect(borders?.right?.style).toBe("medium");
    expect(borders?.bottom).toBeDefined();
    expect(borders?.left).toBeDefined();
  });

  it("outer preset only edges the selection boundary", () => {
    // Middle cell of a 3x3 selection: no outer edges at all.
    const middle = bordersForPresetCell("outer", undefined, pos({
      isFirstRow: false, isLastRow: false, isFirstCol: false, isLastCol: false,
    }));
    expect(middle).toBeUndefined();

    // Top-row-middle cell: top edge only.
    const topEdge = bordersForPresetCell("outer", undefined, pos({
      isFirstRow: true, isLastRow: false, isFirstCol: false, isLastCol: false,
    }));
    expect(topEdge?.top).toBeDefined();
    expect(topEdge?.bottom).toBeUndefined();
    expect(topEdge?.left).toBeUndefined();
  });

  it("all preset adds inner separators", () => {
    const middle = bordersForPresetCell("all", undefined, pos({
      isFirstRow: false, isLastRow: false, isFirstCol: false, isLastCol: false,
    }));
    expect(middle?.top).toBeDefined();
    expect(middle?.bottom).toBeDefined();
    expect(middle?.left).toBeDefined();
    expect(middle?.right).toBeDefined();
    expect(middle?.top?.style).toBe("thin");
  });

  it("top/bottom presets only touch their edge row", () => {
    const firstRow = bordersForPresetCell("top", undefined, pos({ isLastRow: false }));
    expect(firstRow?.top).toBeDefined();
    expect(firstRow?.bottom).toBeUndefined();
    const notFirst = bordersForPresetCell("top", undefined, pos({ isFirstRow: false }));
    expect(notFirst).toBeUndefined();
    const lastRow = bordersForPresetCell("bottom", undefined, pos({ isFirstRow: false }));
    expect(lastRow?.bottom).toBeDefined();
  });

  it("none preset strips borders from the style", () => {
    const styled = applyBorderPresetToStyle({ bold: true, borders: { top: { style: "thin" } } }, "none", undefined, pos());
    expect(styled.borders).toBeUndefined();
    expect(styled.bold).toBe(true);
    // Style without borders passes through untouched.
    expect(applyBorderPresetToStyle({ bold: true }, "none", undefined, pos())).toEqual({ bold: true });
  });

  it("applyBorderPresetToStyle preserves other style fields", () => {
    const styled = applyBorderPresetToStyle({ bgColor: "#eeeeee" }, "all", { style: "dashed" }, pos());
    expect(styled.bgColor).toBe("#eeeeee");
    expect(styled.borders?.top?.style).toBe("dashed");
  });

  it("rangePosition flags the four selection boundaries", () => {
    const bounds = { startRow: 2, endRow: 4, startCol: 3, endCol: 5 } as const;
    const topLeft = rangePosition(bounds as any, 2, 3);
    expect(topLeft.isFirstRow && topLeft.isFirstCol).toBe(true);
    expect(topLeft.isLastRow || topLeft.isLastCol).toBe(false);
    const bottomRight = rangePosition(bounds as any, 4, 5);
    expect(bottomRight.isLastRow && bottomRight.isLastCol).toBe(true);
  });
});
