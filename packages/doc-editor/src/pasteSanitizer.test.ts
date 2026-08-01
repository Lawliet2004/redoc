import { describe, expect, it } from "vitest";
import { transformPastedHTML } from "./pasteSanitizer";

describe("transformPastedHTML", () => {
  it("strips script tags", () => {
    const html = '<p>Hello</p><script>alert("xss")</script><p>World</p>';
    const result = transformPastedHTML(html);
    expect(result).not.toContain("<script");
    expect(result).toContain("Hello");
    expect(result).toContain("World");
  });

  it("strips style tags", () => {
    const html = "<style>body { display: none; }</style><p>Visible</p>";
    const result = transformPastedHTML(html);
    expect(result).not.toContain("<style");
    expect(result).toContain("Visible");
  });

  it("removes on* event handler attributes", () => {
    const html = '<p onclick="evil()" onmouseover="bad()">Click me</p>';
    const result = transformPastedHTML(html);
    expect(result).not.toMatch(/onclick/i);
    expect(result).not.toMatch(/onmouseover/i);
    expect(result).toContain("Click me");
  });

  it("removes inline style attributes", () => {
    const html = '<span style="color:red">Styled</span>';
    const result = transformPastedHTML(html);
    expect(result).not.toMatch(/\bstyle=/i);
    expect(result).toContain("Styled");
  });

  it("returns empty string for blank input", () => {
    expect(transformPastedHTML("")).toBe("");
    expect(transformPastedHTML("   ")).toBe("");
  });
});
