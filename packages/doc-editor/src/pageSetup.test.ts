import { describe, expect, it } from "vitest";
import { DEFAULT_PAGE_SETUP, MAX_PAGE_SETUP_TEXT, formatPrintHeaderFooter, normalizePageSetupConfig, normalizePreviewPageCount } from "./pageSetup";

describe("page setup normalization", () => {
  it("uses safe defaults for malformed metadata", () => {
    expect(normalizePageSetupConfig(null)).toEqual(DEFAULT_PAGE_SETUP);
    expect(normalizePageSetupConfig({ margins: "bad", orientation: "sideways", paperSize: "tabloid", columns: NaN })).toEqual(DEFAULT_PAGE_SETUP);
  });

  it("bounds margins and columns while preserving valid values", () => {
    expect(normalizePageSetupConfig({
      margins: { top: -2, bottom: 20, left: 1.25, right: Infinity },
      orientation: "landscape",
      paperSize: "a4",
      columns: 3.9,
    })).toEqual({
      margins: { top: 0, bottom: 12, left: 1.25, right: 1 },
      orientation: "landscape",
      paperSize: "a4",
      columns: 3,
    });
  });

  it("accepts case-insensitive imported paper and orientation values", () => {
    expect(normalizePageSetupConfig({ paperSize: "A4", orientation: "LANDSCAPE" })).toMatchObject({
      paperSize: "a4",
      orientation: "landscape",
    });
  });

  it("caps header and footer text to a bounded size", () => {
    const value = normalizePageSetupConfig({ header: "h".repeat(MAX_PAGE_SETUP_TEXT + 10), footer: 42 });
    expect(value.header).toHaveLength(MAX_PAGE_SETUP_TEXT);
    expect(value.footer).toBeUndefined();
  });

  it("bounds preview page counts and substitutes print fields safely", () => {
    expect(normalizePreviewPageCount(Number.POSITIVE_INFINITY)).toBe(1);
    expect(normalizePreviewPageCount(20_001)).toBe(10_000);
    expect(formatPrintHeaderFooter("Page {page} of {pages} ({total})", 4, 2)).toBe("Page 2 of 4 (4)");
    expect(formatPrintHeaderFooter("x".repeat(MAX_PAGE_SETUP_TEXT + 10), 2)).toHaveLength(MAX_PAGE_SETUP_TEXT);
    expect(formatPrintHeaderFooter(42, 2)).toBe("");
  });
});
