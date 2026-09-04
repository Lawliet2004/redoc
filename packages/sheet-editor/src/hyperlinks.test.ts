import { describe, expect, it } from "vitest";
import { normalizeSheetHyperlink, parseInternalSheetLocation } from "./hyperlinks";

describe("sheet hyperlinks", () => {
  it("accepts supported external links", () => {
    expect(normalizeSheetHyperlink(" https://example.test/report ")).toBe("https://example.test/report");
    expect(normalizeSheetHyperlink("mailto:finance@example.test")).toBe("mailto:finance@example.test");
    expect(normalizeSheetHyperlink("tel:+15551212")).toBe("tel:+15551212");
    expect(normalizeSheetHyperlink("https://")).toBeNull();
  });

  it("canonicalizes internal locations and rejects unsafe targets", () => {
    expect(normalizeSheetHyperlink("#Sheet2!B4")).toBe("internal:Sheet2!B4");
    expect(normalizeSheetHyperlink("internal:Sheet2!B4")).toBe("internal:Sheet2!B4");
    expect(normalizeSheetHyperlink("javascript:alert(1)")).toBeNull();
    expect(normalizeSheetHyperlink("data:text/html,unsafe")).toBeNull();
    expect(parseInternalSheetLocation("internal:'Sheet 2'!$B$4")).toEqual({ sheetName: "Sheet 2", row: 4, col: 2 });
    expect(parseInternalSheetLocation("internal:A100001")).toBeNull();
  });
});
