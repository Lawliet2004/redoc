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

  it("strips javascript, vbscript, and data hyperlinks", () => {
    const html = '<a href="javascript:alert(1)">x</a><a href="vbscript:msgbox(1)">y</a><a href="data:text/html,hi">z</a><a href="https://example.com">ok</a>';
    const result = transformPastedHTML(html);
    expect(result).not.toMatch(/javascript:/i);
    expect(result).not.toMatch(/vbscript:/i);
    expect(result).not.toMatch(/data:text\/html/i);
    expect(result).toContain("https://example.com");
  });

  it("keeps raster data-image sources and drops HTML data URLs", () => {
    const html = '<img src="data:image/png;base64,AAAA"><img src="data:text/html,<script>alert(1)</script>">';
    const result = transformPastedHTML(html);
    expect(result).toContain("data:image/png;base64,AAAA");
    expect(result).not.toMatch(/data:text\/html/i);
  });
});
