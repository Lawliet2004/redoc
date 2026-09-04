import { describe, expect, it } from "vitest";
import {
  BUILT_IN_STYLES,
  customStylesForDoc,
  findStyle,
  normalizeStyleName,
  resolveStyles,
  styleBlockAttrs,
  styleIdFromName,
  styleInlineAttrs,
  wordStyleId,
} from "./styles";
import type { DocContent } from "./types";

describe("named paragraph styles", () => {
  it("resolves built-in styles and bounded custom styles from a doc", () => {
    const doc = {
      type: "doc",
      styles: [
        { id: "Custom1", name: "My Style", blockType: "paragraph", builtIn: false },
        { id: "", name: "", blockType: "paragraph", builtIn: false },
      ],
    } as unknown as DocContent;
    const styles = resolveStyles(doc);
    expect(styles.length).toBe(BUILT_IN_STYLES.length + 1);
    expect(findStyle(doc, "My Style")?.id).toBe("Custom1");
    expect(findStyle(doc, "Title")?.name).toBe("Title");
  });

  it("normalizes and caps custom style names", () => {
    expect(normalizeStyleName("  Hello \u0000 World  ")).toBe("Hello  World");
    expect(customStylesForDoc(
      Array.from({ length: 100 }, (_, i) => ({
        id: `S${i}`,
        name: `Style ${i}`,
        blockType: "paragraph" as const,
        builtIn: false,
      })),
    ).length).toBeLessThanOrEqual(64);
  });

  it("derives block attrs and inline marks from a style", () => {
    const title = findStyle(null, "Title")!;
    const attrs = styleBlockAttrs(title);
    expect(attrs.styleName).toBe("Title");
    expect(attrs.align).toBe("center");
    expect(attrs.spacingAfter).toBe(6);
    const marks = styleInlineAttrs(title);
    expect(marks.map((m) => m.type)).toEqual(["bold", "fontSize"]);
    expect(styleIdFromName("My Cool Style!")).toBe("MyCoolStyle");
  });

  it("maps styles to Word pStyle ids", () => {
    expect(wordStyleId("Title")).toBe("Title");
    expect(wordStyleId("Normal")).toBeNull();
    expect(wordStyleId("Heading1", 2)).toBe("Heading2");
  });
});
