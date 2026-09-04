import { describe, expect, it } from "vitest";
import { documentFieldDisplay, documentFieldLabel, documentFieldResult, normalizeDocumentFieldKind } from "./fields";

describe("document fields", () => {
  it("normalizes supported page field aliases", () => {
    expect(normalizeDocumentFieldKind("PAGE")).toBe("page");
    expect(normalizeDocumentFieldKind("pageNumber")).toBe("page");
    expect(normalizeDocumentFieldKind("NUMPAGES")).toBe("numPages");
    expect(normalizeDocumentFieldKind("unsupported")).toBeNull();
  });

  it("uses a bounded result or a visible fallback label", () => {
    expect(documentFieldLabel("numPages")).toBe("NUMPAGES");
    expect(documentFieldDisplay("page", 3)).toBe("3");
    expect(documentFieldDisplay("numPages", "")).toBe("[NUMPAGES]");
    expect(documentFieldDisplay("page", "x".repeat(100))).toHaveLength(64);
  });

  it("clamps refreshed field results to safe page bounds", () => {
    expect(documentFieldResult("page", 4, 2)).toBe("2");
    expect(documentFieldResult("numPages", 4, 2)).toBe("4");
    expect(documentFieldResult("page", 0, Number.POSITIVE_INFINITY)).toBe("1");
    expect(documentFieldResult("numPages", 2_000_000, 1)).toBe("1000000");
  });
});
