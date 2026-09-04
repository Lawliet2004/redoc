import { describe, expect, it } from "vitest";
import { normalizeSlideHyperlink } from "./hyperlinks";

describe("slide hyperlinks", () => {
  it("accepts bounded safe external schemes", () => {
    expect(normalizeSlideHyperlink(" https://example.com/docs ")).toBe("https://example.com/docs");
    expect(normalizeSlideHyperlink("mailto:author@example.com")).toBe("mailto:author@example.com");
    expect(normalizeSlideHyperlink("tel:+1 (555) 010-1234")).toBe("tel:+1 (555) 010-1234");
  });

  it("rejects unsafe, malformed, and oversized targets", () => {
    expect(normalizeSlideHyperlink("javascript:alert(1)")).toBeNull();
    expect(normalizeSlideHyperlink("https://")).toBeNull();
    expect(normalizeSlideHyperlink(`https://example.com/${"x".repeat(2_050)}`)).toBeNull();
    expect(normalizeSlideHyperlink("https://example.com/\nnext")).toBeNull();
  });
});
