import type { DocContent } from "./types";

/** Named paragraph styles recognized by the editor and Word export. */
export interface NamedStyleDef {
  /** Internal id used as the DOCX pStyle name (kept Word-compatible). */
  id: string;
  name: string;
  /** Base ProseMirror block type this style applies. */
  blockType: "paragraph" | "heading";
  headingLevel?: number;
  /** Formatting applied when the style is used. */
  align?: "left" | "center" | "right" | "justify";
  bold?: boolean;
  italic?: boolean;
  color?: string;
  fontSize?: number;
  spacingBefore?: number;
  spacingAfter?: number;
  builtIn: boolean;
}

export const BUILT_IN_STYLES: NamedStyleDef[] = [
  { id: "Normal", name: "Default Paragraph Style", blockType: "paragraph", builtIn: true },
  { id: "Title", name: "Title", blockType: "paragraph", align: "center", bold: true, fontSize: 28, spacingAfter: 6, builtIn: true },
  { id: "Subtitle", name: "Subtitle", blockType: "paragraph", align: "center", color: "#475569", fontSize: 15, spacingAfter: 12, builtIn: true },
  { id: "Quote", name: "Quote", blockType: "paragraph", italic: true, color: "#64748b", builtIn: true },
  { id: "Intense", name: "Intense Emphasis", blockType: "paragraph", bold: true, builtIn: true },
];

export const HEADING_STYLE_IDS = ["Heading1", "Heading2", "Heading3", "Heading4", "Heading5", "Heading6"] as const;

export const MAX_CUSTOM_STYLES = 64;
export const MAX_STYLE_NAME_LENGTH = 60;

export function normalizeStyleName(value: unknown): string {
  return String(value ?? "")
    .replace(/[\u0000-\u001f]/g, "")
    .trim()
    .slice(0, MAX_STYLE_NAME_LENGTH);
}

export function styleIdFromName(name: string): string {
  const id = name.replace(/[^A-Za-z0-9]/g, "") || "Custom";
  return id.length > 40 ? id.slice(0, 40) : id;
}

/** All style definitions available to a document (built-ins + custom). */
export function resolveStyles(doc: DocContent | null | undefined): NamedStyleDef[] {
  const custom = Array.isArray((doc as Record<string, unknown> | null | undefined)?.styles)
    ? ((doc as Record<string, unknown>).styles as NamedStyleDef[])
        .filter((style) => style && typeof style.id === "string" && typeof style.name === "string" && style.name.trim().length > 0)
        .slice(0, MAX_CUSTOM_STYLES)
    : [];
  return [...BUILT_IN_STYLES, ...custom];
}

export function findStyle(doc: DocContent | null | undefined, styleName: string): NamedStyleDef | undefined {
  return resolveStyles(doc).find((style) => style.id === styleName || style.name === styleName);
}

/** Paragraph formatting attrs implied by a named style. */
export function styleBlockAttrs(style: NamedStyleDef): Record<string, string | number> {
  const attrs: Record<string, string | number> = {
    styleName: style.id,
  };
  if (style.align) attrs.align = style.align;
  if (style.spacingBefore !== undefined) attrs.spacingBefore = style.spacingBefore;
  if (style.spacingAfter !== undefined) attrs.spacingAfter = style.spacingAfter;
  if (style.blockType === "heading") attrs.level = style.headingLevel ?? 1;
  return attrs;
}

/** Mark attributes implied by a named style's inline formatting. */
export function styleInlineAttrs(style: NamedStyleDef): Array<{ type: string; attrs: Record<string, string> }> {
  const marks: Array<{ type: string; attrs: Record<string, string> }> = [];
  if (style.bold) marks.push({ type: "bold", attrs: {} });
  if (style.italic) marks.push({ type: "italic", attrs: {} });
  if (style.color) marks.push({ type: "color", attrs: { color: style.color } });
  if (style.fontSize) marks.push({ type: "fontSize", attrs: { size: String(style.fontSize) } });
  return marks;
}

/**
 * Build the `styles` array for persistence: custom styles only, normalized.
 * Built-in styles are implied and never stored.
 */
export function customStylesForDoc(styles: NamedStyleDef[]): NamedStyleDef[] {
  return styles
    .filter((style) => !style.builtIn)
    .map((style) => ({
      ...style,
      id: normalizeStyleName(style.id) || styleIdFromName(style.name),
      name: normalizeStyleName(style.name),
    }))
    .filter((style) => style.name.length > 0)
    .slice(0, MAX_CUSTOM_STYLES);
}

/** Map a style id to the Word pStyle name used by DOCX export. */
export function wordStyleId(styleName: string, headingLevel?: number): string | null {
  if (headingLevel && HEADING_STYLE_IDS.includes(`Heading${headingLevel}` as (typeof HEADING_STYLE_IDS)[number])) {
    return `Heading${headingLevel}`;
  }
  const match = BUILT_IN_STYLES.find((style) => style.id === styleName);
  return match && match.id !== "Normal" ? match.id : null;
}
