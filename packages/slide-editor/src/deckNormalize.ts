import { isShapeType, type ShapeType } from "./shapeUtils";

export const PLACEHOLDER_TITLE = "Click to add Title";
export const PLACEHOLDER_BODY = "Click to add Text";

/** Collision-resistant element/slide id; falls back when crypto.randomUUID
 * is unavailable (older WebViews, insecure contexts). */
export function generateSlideId(prefix: string): string {
  if (typeof crypto !== "undefined" && typeof crypto.randomUUID === "function") {
    return `${prefix}-${crypto.randomUUID()}`;
  }
  return `${prefix}-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 10)}`;
}

export type SlideTransition = "none" | "fade" | "slide-left" | "slide-right" | "wipe-left" | "wipe-right" | "zoom" | "dissolve" | "morph";
export type ElementEntrance = "none" | "fade" | "zoom";
export type ElementExit = "none" | "fade";

export interface TableMerge {
  r: number;
  c: number;
  rowspan: number;
  colspan: number;
}

export type TableStyle = "plain" | "banded" | "accent-header";

export interface ShapeGradient {
  from: string;
  to: string;
  angle?: number;
}

export interface SlideElement {
  id: string;
  type: "text" | ShapeType | "image" | "table" | "chart" | "audio" | "video";
  x: number;
  y: number;
  width: number;
  height: number;
  content: string;
  fontSize?: number;
  color?: string;
  rotation?: number;
  align?: "left" | "center" | "right";
  bullets?: boolean;
  entrance?: ElementEntrance;
  entranceDelayMs?: number;
  entranceDurationMs?: number;
  entranceOrder?: number;
  exit?: ElementExit;
  exitDurationMs?: number;
  hyperlink?: string;
  fontFamily?: string;
  bold?: boolean;
  italic?: boolean;
  underline?: boolean;
  groupId?: string;
  tableRows?: number;
  tableCols?: number;
  tableData?: string[][];
  tableMerges?: TableMerge[];
  tableHeaderRow?: boolean;
  tableStyle?: TableStyle;
  chartType?: "bar" | "pie" | "line";
  chartTitle?: string;
  chartData?: number[];
  chartLabels?: string[];
  chartLegend?: boolean;
  chartShowLabels?: boolean;
  chartShowAxes?: boolean;
  /** Gradient fill for closed shapes (fill vs line are independent). */
  fillGradient?: ShapeGradient;
  /** Independent line/stroke color for closed shapes (lines/arrows use color). */
  lineColor?: string;
  lineWidth?: number;
  shadow?: boolean;
  mediaMime?: string;
  /** True for scaffolded placeholder text — safe to replace when applying a layout. */
  placeholder?: boolean;
}

export interface SlideComment {
  id: string;
  author: string;
  text: string;
  resolved: boolean;
  createdAt: string;
}

export interface SlideMaster {
  id: string;
  name: string;
  bgColor?: string;
  textColor?: string;
  accentColor?: string;
  fontFamily?: string;
  showHeader?: boolean;
  headerText?: string;
  showFooter?: boolean;
  footerText?: string;
  showDate?: boolean;
  dateText?: string;
  showSlideNumber?: boolean;
}

export interface Slide {
  id: string;
  title: string;
  layout: string;
  elements: SlideElement[];
  notes: string;
  transition: SlideTransition;
  bgOverride?: string | null;
  comments?: SlideComment[];
  masterId?: string;
  showHeader?: boolean;
  showFooter?: boolean;
  showDate?: boolean;
  showSlideNumber?: boolean;
}

/** System-only font stack for deck themes — no remote font imports (offline). */
export const SLIDE_FONT_STACK = 'system-ui, -apple-system, "Segoe UI", Roboto, "Helvetica Neue", Arial, "Noto Sans", sans-serif';

export const defaultMasters: SlideMaster[] = [
  {
    id: "master-default",
    name: "Default",
    bgColor: "#ffffff",
    textColor: "#0f172a",
    accentColor: "#4f46e5",
    fontFamily: SLIDE_FONT_STACK,
    showHeader: false,
    showFooter: true,
    footerText: "",
    showDate: false,
    dateText: "",
    showSlideNumber: true,
  },
  {
    id: "master-midnight",
    name: "Midnight",
    bgColor: "#0d1117",
    textColor: "#e6edf3",
    accentColor: "#818cf8",
    fontFamily: SLIDE_FONT_STACK,
    showHeader: false,
    showFooter: true,
    footerText: "",
    showDate: true,
    dateText: "",
    showSlideNumber: true,
  },
  {
    id: "master-corporate",
    name: "Corporate",
    bgColor: "#f8fafc",
    textColor: "#0f172a",
    accentColor: "#0f766e",
    fontFamily: "Arial, Helvetica, sans-serif",
    showHeader: true,
    headerText: "Company",
    showFooter: true,
    footerText: "Confidential",
    showDate: true,
    dateText: "",
    showSlideNumber: true,
  },
];

/** Normalize a slide-master list; unknown entries fall back to entry defaults. */
export function normalizeMasters(value: unknown): SlideMaster[] {
  if (!Array.isArray(value) || value.length === 0) return defaultMasters.map((m) => ({ ...m }));
  const out: SlideMaster[] = [];
  for (const entry of value.slice(0, 32)) {
    if (!entry || typeof entry !== "object") continue;
    const rec = entry as Record<string, unknown>;
    const id = typeof rec.id === "string" && rec.id.trim() ? rec.id.trim().slice(0, 80) : `master-${out.length + 1}`;
    const name = typeof rec.name === "string" && rec.name.trim() ? rec.name.trim().slice(0, 80) : id;
    out.push({
      id,
      name,
      bgColor: normalizeHexColor(rec.bgColor) ?? undefined,
      textColor: normalizeHexColor(rec.textColor) ?? undefined,
      accentColor: normalizeHexColor(rec.accentColor) ?? undefined,
      fontFamily: typeof rec.fontFamily === "string" ? rec.fontFamily.slice(0, 120) : undefined,
      showHeader: rec.showHeader === true,
      headerText: typeof rec.headerText === "string" ? rec.headerText.slice(0, 200) : undefined,
      showFooter: rec.showFooter === true,
      footerText: typeof rec.footerText === "string" ? rec.footerText.slice(0, 200) : undefined,
      showDate: rec.showDate === true,
      dateText: typeof rec.dateText === "string" ? rec.dateText.slice(0, 80) : undefined,
      showSlideNumber: rec.showSlideNumber !== false,
    });
  }
  return out.length ? out : defaultMasters.map((m) => ({ ...m }));
}

export function normalizeHexColor(value: unknown): string | null {
  if (typeof value !== "string") return null;
  const v = value.trim();
  if (/^#[0-9a-fA-F]{6}$/.test(v)) return v;
  if (/^#[0-9a-fA-F]{3}$/.test(v)) {
    return `#${v[1]}${v[1]}${v[2]}${v[2]}${v[3]}${v[3]}`;
  }
  return null;
}

/** Normalize a gradient spec; malformed specs are dropped (no gradient). */
export function normalizeGradient(value: unknown): ShapeGradient | undefined {
  if (!value || typeof value !== "object") return undefined;
  const rec = value as Record<string, unknown>;
  const from = normalizeHexColor(rec.from);
  const to = normalizeHexColor(rec.to);
  if (!from || !to) return undefined;
  const angle = Number(rec.angle);
  return { from, to, angle: Number.isFinite(angle) ? Math.max(0, Math.min(360, Math.round(angle))) : 90 };
}

export function normalizeTableMerges(value: unknown, rows: number, cols: number): TableMerge[] {
  if (!Array.isArray(value)) return [];
  const out: TableMerge[] = [];
  for (const entry of value.slice(0, 256)) {
    if (!entry || typeof entry !== "object") continue;
    const rec = entry as Record<string, unknown>;
    const r = Math.floor(Number(rec.r));
    const c = Math.floor(Number(rec.c));
    const rowspan = Math.floor(Number(rec.rowspan));
    const colspan = Math.floor(Number(rec.colspan));
    if (!Number.isFinite(r) || !Number.isFinite(c) || !Number.isFinite(rowspan) || !Number.isFinite(colspan)) continue;
    if (r < 0 || c < 0 || r >= rows || c >= cols) continue;
    if (rowspan < 1 || colspan < 1) continue;
    if (r + rowspan > rows || c + colspan > cols) continue;
    if (rowspan === 1 && colspan === 1) continue;
    out.push({ r, c, rowspan, colspan });
  }
  return out;
}

export function normalizeTableStyle(value: unknown): TableStyle {
  return value === "banded" || value === "accent-header" ? value : "plain";
}

/** Resolve a slide's effective header/footer/date/number via master inheritance. */
export function resolveSlideChrome(
  slide: Pick<Slide, "showHeader" | "showFooter" | "showDate" | "showSlideNumber" | "masterId">,
  masters: SlideMaster[],
  index: number,
): { header: string | null; footer: string | null; date: string | null; number: string | null } {
  const master = masters.find((m) => m.id === slide.masterId) ?? masters[0];
  const showHeader = slide.showHeader ?? master?.showHeader ?? false;
  const showFooter = slide.showFooter ?? master?.showFooter ?? false;
  const showDate = slide.showDate ?? master?.showDate ?? false;
  const showNumber = slide.showSlideNumber ?? master?.showSlideNumber ?? false;
  return {
    header: showHeader ? (master?.headerText?.trim() ? master.headerText : null) : null,
    footer: showFooter ? (master?.footerText?.trim() ? master.footerText : " ") : null,
    date: showDate ? (master?.dateText?.trim() ? master.dateText : new Date().toISOString().slice(0, 10)) : null,
    number: showNumber ? String(index + 1) : null,
  };
}

/** Master chrome as concrete overlay boxes for editor/presenter rendering. */
export function slideChromeOverlays(
  slide: Slide,
  masters: SlideMaster[],
  index: number,
  canvasWidth: number,
  canvasHeight: number,
): Array<{ key: string; text: string; x: number; y: number; width: number; fontSize: number; align: "left" | "center" | "right" }> {
  const chrome = resolveSlideChrome(slide, masters, index);
  const overlays: Array<{ key: string; text: string; x: number; y: number; width: number; fontSize: number; align: "left" | "center" | "right" }> = [];
  const footerY = Math.max(20, canvasHeight - 28);
  if (chrome.header) {
    overlays.push({ key: "header", text: chrome.header, x: 40, y: 8, width: canvasWidth - 80, fontSize: 12, align: "center" });
  }
  if (chrome.footer) {
    overlays.push({ key: "footer", text: chrome.footer, x: 40, y: footerY, width: canvasWidth / 3 - 60, fontSize: 11, align: "left" });
  }
  if (chrome.date) {
    overlays.push({ key: "date", text: chrome.date, x: canvasWidth / 3, y: footerY, width: canvasWidth / 3, fontSize: 11, align: "center" });
  }
  if (chrome.number) {
    overlays.push({ key: "number", text: chrome.number, x: canvasWidth - 100, y: footerY, width: 60, fontSize: 11, align: "right" });
  }
  return overlays;
}

/** Bounded per-slide comment list for the review sidebar. */
export function normalizeSlideComments(value: unknown): SlideComment[] {
  if (!Array.isArray(value)) return [];
  return value
    .filter((entry): entry is Record<string, unknown> => Boolean(entry) && typeof entry === "object")
    .slice(0, 200)
    .map((entry) => ({
      id: typeof entry.id === "string" ? entry.id : `comment-${Math.random().toString(36).slice(2, 10)}`,
      author: typeof entry.author === "string" && entry.author.trim() ? entry.author.trim().slice(0, 80) : "You",
      text: typeof entry.text === "string" ? entry.text.slice(0, 2_000) : "",
      resolved: entry.resolved === true,
      createdAt: typeof entry.createdAt === "string" ? entry.createdAt : new Date().toISOString(),
    }))
    .filter((comment) => comment.text.trim().length > 0);
}

export interface SlideTheme {
  id: string;
  name: string;
  bgColor: string;
  textColor: string;
  accentColor: string;
  fontFamily: string;
}

export const defaultTheme: SlideTheme = {
  id: "default-light",
  name: "Modern Light",
  bgColor: "#ffffff",
  textColor: "#0f172a",
  accentColor: "#4f46e5",
  fontFamily: SLIDE_FONT_STACK,
};

/**
 * Built-in deck themes — contemporary, muted palettes in the spirit of
 * Linear/Stripe/Notion. Keys are stable preset ids used by the Styles panel
 * and `setThemePreset`; `light` aliases `defaultTheme`.
 */
export const SLIDE_THEME_PRESETS: Record<string, SlideTheme> = {
  light: defaultTheme,
  midnight: {
    id: "midnight",
    name: "Midnight",
    bgColor: "#0d1117",
    textColor: "#e6edf3",
    accentColor: "#818cf8",
    fontFamily: SLIDE_FONT_STACK,
  },
  coral: {
    id: "coral",
    name: "Coral",
    bgColor: "#fff6f1",
    textColor: "#41291f",
    accentColor: "#e04e27",
    fontFamily: SLIDE_FONT_STACK,
  },
  forest: {
    id: "forest",
    name: "Forest",
    bgColor: "#f2f7f4",
    textColor: "#0e2a1e",
    accentColor: "#15803d",
    fontFamily: SLIDE_FONT_STACK,
  },
  lavender: {
    id: "lavender",
    name: "Lavender",
    bgColor: "#f7f5fb",
    textColor: "#272244",
    accentColor: "#6d28d9",
    fontFamily: SLIDE_FONT_STACK,
  },
  sunset: {
    id: "sunset",
    name: "Sunset",
    bgColor: "#1b1410",
    textColor: "#f3e9e0",
    accentColor: "#fb923c",
    fontFamily: SLIDE_FONT_STACK,
  },
};

export function normalizeTransition(value: unknown, fallbackFade = false): SlideTransition {
  if (
    value === "fade" || value === "slide-left" || value === "slide-right" ||
    value === "wipe-left" || value === "wipe-right" || value === "zoom" ||
    value === "dissolve" || value === "morph" || value === "none"
  ) return value;
  return fallbackFade ? "fade" : "none";
}

export function normalizeEntrance(value: unknown): ElementEntrance {
  return value === "fade" || value === "zoom" ? value : "none";
}

export function normalizeExit(value: unknown): ElementExit {
  return value === "fade" ? value : "none";
}

export function normalizeTextAlign(value: unknown): "left" | "center" | "right" {
  return value === "left" || value === "right" ? value : "center";
}

function normalizeEntranceTiming(element: any, fallbackOrder = 0) {
  return {
    entranceDelayMs: Number.isFinite(element.entranceDelayMs) ? Math.max(0, Number(element.entranceDelayMs)) : 0,
    entranceDurationMs: Number.isFinite(element.entranceDurationMs) ? Math.max(50, Number(element.entranceDurationMs)) : 350,
    entranceOrder: Number.isFinite(element.entranceOrder) ? Math.max(0, Number(element.entranceOrder)) : fallbackOrder,
    exit: normalizeExit(element.exit),
    exitDurationMs: Number.isFinite(element.exitDurationMs) ? Math.max(50, Number(element.exitDurationMs)) : 350,
  };
}

/** Bounded group id: grouping applies to every element kind, not just shapes. */
function normalizeGroupId(value: unknown): string | undefined {
  return typeof value === "string" && value.trim() ? value.trim().slice(0, 80) : undefined;
}

/** True only for explicitly flagged scaffold placeholders (never user text). */
function normalizePlaceholderFlag(value: unknown): true | undefined {
  return value === true ? true : undefined;
}

export function normalizeFlatElement(element: any, rotation: number, entrance: ElementEntrance, fallbackOrder = 0): SlideElement | null {
  const type = element.type as string;
  const link = typeof element.hyperlink === "string" ? element.hyperlink : undefined;
  const style = {
    fillGradient: normalizeGradient(element.fillGradient),
    lineColor: normalizeHexColor(element.lineColor) ?? undefined,
    lineWidth: Number.isFinite(element.lineWidth) ? Math.max(0, Math.min(24, Number(element.lineWidth))) : undefined,
    shadow: element.shadow === true ? true : undefined,
    fontFamily: typeof element.fontFamily === "string" ? element.fontFamily.slice(0, 120) : undefined,
    bold: element.bold === true ? true : undefined,
    italic: element.italic === true ? true : undefined,
    underline: element.underline === true ? true : undefined,
  };
  const base = {
    id: element.id || `el-${Math.random()}`,
    x: typeof element.x === "number" ? element.x : 100,
    y: typeof element.y === "number" ? element.y : 100,
    width: typeof element.width === "number" ? element.width : 400,
    height: typeof element.height === "number" ? element.height : 60,
    rotation,
    entrance,
    hyperlink: link,
    groupId: normalizeGroupId(element.groupId),
    placeholder: normalizePlaceholderFlag(element.placeholder),
    ...normalizeEntranceTiming(element, fallbackOrder),
  };
  if (type === "text") {
    return {
      ...base,
      type: "text",
      content: element.content ?? "",
      fontSize: element.fontSize,
      color: element.color ?? "#1e293b",
      align: normalizeTextAlign(element.align),
      bullets: Boolean(element.bullets),
      fontFamily: style.fontFamily,
      bold: Boolean(element.bold),
      italic: Boolean(element.italic),
      underline: Boolean(element.underline),
    };
  }
  if (type === "image") {
    return {
      ...base,
      type: "image",
      content: element.content ?? "",
      color: element.color ?? "#ffffff",
    };
  }
  if (type === "audio" || type === "video") {
    return {
      ...base,
      type,
      content: typeof element.content === "string" ? element.content.slice(0, 4_096) : "",
      color: element.color ?? "#0f172a",
      mediaMime: typeof element.mediaMime === "string" ? element.mediaMime.slice(0, 80) : undefined,
    };
  }
  if (type === "table") {
    const data = normalizeTableData(element.tableData, element.tableRows, element.tableCols);
    return {
      ...base,
      type: "table",
      content: "",
      tableRows: data.length,
      tableCols: data[0]?.length ?? 0,
      tableData: data,
      tableMerges: normalizeTableMerges(element.tableMerges, data.length, data[0]?.length ?? 0),
      tableHeaderRow: element.tableHeaderRow === true ? true : undefined,
      tableStyle: normalizeTableStyle(element.tableStyle),
    };
  }
  if (type === "chart") {
    return {
      ...base,
      type: "chart",
      content: "",
      chartType: normalizeChartType(element.chartType),
      chartTitle: typeof element.chartTitle === "string" ? element.chartTitle.slice(0, 200) : "Chart",
      chartData: normalizeChartData(element.chartData),
      chartLabels: normalizeChartLabels(element.chartLabels),
      chartLegend: element.chartLegend !== false ? undefined : false,
      chartShowLabels: element.chartShowLabels === false ? false : undefined,
      chartShowAxes: element.chartShowAxes === false ? false : undefined,
    };
  }
  if (type === "rect" || type === "ellipse" || type === "line" || type === "arrow"
    || type === "roundedRect" || type === "triangle" || type === "diamond" || type === "star") {
    return {
      ...base,
      type,
      content: element.content ?? "",
      color: element.color ?? "#3b82f6",
      ...style,
    };
  }
  return null;
}

/** Normalize free-form table data into a bounded rectangular grid. */
export function normalizeTableData(value: unknown, rows?: unknown, cols?: unknown): string[][] {
  let data: string[][] = [];
  if (Array.isArray(value)) {
    data = value.slice(0, 20).map((row) =>
      (Array.isArray(row) ? row : []).slice(0, 12).map((cell) => String(cell ?? "").slice(0, 500)),
    );
  }
  const wantRows = Math.max(1, Math.min(20, Math.floor(Number(rows)) || data.length || 3));
  const wantCols = Math.max(1, Math.min(12, Math.floor(Number(cols)) || data[0]?.length || 3));
  while (data.length < wantRows) data.push([]);
  data = data.slice(0, wantRows).map((row) => {
    const next = [...row];
    while (next.length < wantCols) next.push("");
    return next.slice(0, wantCols);
  });
  return data;
}

export function normalizeChartType(value: unknown): "bar" | "pie" | "line" {
  return value === "pie" || value === "line" ? value : "bar";
}

export function normalizeChartData(value: unknown): number[] {
  if (!Array.isArray(value)) return [3, 5, 2, 8];
  const data = value.slice(0, 24).map((v) => {
    const n = Number(v);
    return Number.isFinite(n) ? Math.max(-1e9, Math.min(1e9, n)) : 0;
  });
  return data.length ? data : [3, 5, 2, 8];
}

export const CHART_TYPES = ["bar", "line", "pie"] as const;
export type ChartType = (typeof CHART_TYPES)[number];

export function isChartType(value: unknown): value is ChartType {
  return value === "bar" || value === "line" || value === "pie";
}

/** Per-category color matching the editor/presenter/PDF hsl(i*47) palette. */
export function chartSeriesColor(index: number): string {
  return `hsl(${(index * 47) % 360}, 65%, 55%)`;
}

/** Bounded deck canvas size; non-finite or non-positive values fall back to 960x540. */
export function normalizeCanvasSize(
  width: unknown,
  height: unknown,
): { width: number; height: number } {
  const parse = (value: unknown, fallback: number) => {
    const n = Number(value);
    return Number.isFinite(n) && n > 0 ? Math.min(5120, Math.max(320, Math.round(n))) : fallback;
  };
  return { width: parse(width, 960), height: parse(height, 540) };
}

export function normalizeChartLabels(value: unknown): string[] {
  if (!Array.isArray(value)) return ["A", "B", "C", "D"];
  return value.slice(0, 24).map((label) => String(label ?? "").slice(0, 80));
}

/** Parse a slide-internal link (`slide:<id>`, `#<id>`, `internal:<id>`). */
export function parseInternalSlideId(value: string): string | null {
  const match = value.trim().match(/^(?:slide:|internal:|#)(.+)$/i);
  if (!match) return null;
  const id = match[1].trim().slice(0, 120);
  return /^[A-Za-z0-9][A-Za-z0-9\-_:.]*$/.test(id) ? id : null;
}

export function normalizeDeck(deck: any): Slide[] {
  if (!deck?.slides?.length) return [{
    id: "slide-1", title: "Title Slide", layout: "title", notes: "", transition: "none", elements: [
      { id: "el-1", type: "text", x: 100, y: 180, width: 760, height: 80, content: PLACEHOLDER_TITLE, fontSize: 40, color: "#0f172a", rotation: 0, align: "center", placeholder: true },
      { id: "el-2", type: "text", x: 150, y: 280, width: 660, height: 50, content: PLACEHOLDER_BODY, fontSize: 22, color: "#475569", rotation: 0, align: "center", placeholder: true },
    ],
  }];
  const deckFade = Boolean(deck.fadeBetweenSlides);
  const masters = normalizeMasters(deck.masters);
  return deck.slides.map((slide: any, slideIndex: number) => ({
    id: slide.id,
    title: slide.title || slide.layout || "Slide",
    layout: slide.layout || "blank",
    notes: slide.notes || "",
    transition: normalizeTransition(slide.transition, deckFade),
    bgOverride: slide.bgOverride ?? slide.bg_override ?? null,
    comments: normalizeSlideComments(slide.comments),
    masterId: typeof slide.masterId === "string" && slide.masterId ? slide.masterId
      : typeof slide.master_id === "string" && slide.master_id ? slide.master_id
      : masters[0]?.id,
    showHeader: typeof slide.showHeader === "boolean" ? slide.showHeader : undefined,
    showFooter: typeof slide.showFooter === "boolean" ? slide.showFooter : undefined,
    showDate: typeof slide.showDate === "boolean" ? slide.showDate : undefined,
    showSlideNumber: typeof slide.showSlideNumber === "boolean" ? slide.showSlideNumber : undefined,
    elements: (slide.elements || []).map((element: any, elementIndex: number) => {
      const kind = element.kind || {};
      const rotation = typeof element.rotation === "number" ? element.rotation : 0;
      const entrance = normalizeEntrance(element.entrance);
      const timing = normalizeEntranceTiming(element, elementIndex);
      if (!element.kind && element.type) {
        const flat = normalizeFlatElement(element, rotation, entrance, elementIndex);
        if (flat) return flat;
      }
      if (kind.Text) {
        return {
          id: element.id,
          type: "text" as const,
          x: element.x,
          y: element.y,
          width: element.width,
          height: element.height,
          content: kind.Text.text,
          fontSize: kind.Text.fontSize,
          color: kind.Text.color,
          rotation,
          align: normalizeTextAlign(kind.Text.align),
          bullets: Boolean(kind.Text.bullets),
          entrance,
          ...timing,
          hyperlink: typeof element.hyperlink === "string" ? element.hyperlink : undefined,
          groupId: normalizeGroupId(element.groupId),
          placeholder: normalizePlaceholderFlag(element.placeholder),
          fontFamily: kind.Text.fontFamily,
          bold: Boolean(kind.Text.bold),
          italic: Boolean(kind.Text.italic),
          underline: Boolean(kind.Text.underline),
        };
      }
      if (kind.Shape) {
        const shapeType = kind.Shape.shapeType;
        const type: ShapeType = isShapeType(shapeType) ? shapeType : "rect";
        const strokeColor = kind.Shape.strokeColor;
        const color = type === "line" || type === "arrow"
          ? (strokeColor && strokeColor !== "transparent" ? strokeColor : kind.Shape.fillColor || "#1e293b")
          : kind.Shape.fillColor;
        const lineWidth = Number(kind.Shape.strokeWidth);
        return {
          id: element.id,
          type,
          x: element.x,
          y: element.y,
          width: element.width,
          height: element.height,
          content: kind.Shape.text || "",
          color: color || "#3b82f6",
          rotation,
          entrance,
          ...timing,
          hyperlink: typeof element.hyperlink === "string" ? element.hyperlink : undefined,
          groupId: normalizeGroupId(element.groupId),
          placeholder: normalizePlaceholderFlag(element.placeholder),
          fillGradient: normalizeGradient(kind.Shape.fillGradient),
          lineColor: normalizeHexColor(
            type === "line" || type === "arrow" ? undefined : kind.Shape.strokeColor,
          ) ?? undefined,
          lineWidth: Number.isFinite(lineWidth) ? Math.max(0, Math.min(24, lineWidth)) : undefined,
          shadow: kind.Shape.shadow === true ? true : undefined,
          fontFamily: typeof kind.Shape.fontFamily === "string" ? kind.Shape.fontFamily : undefined,
          bold: kind.Shape.bold === true ? true : undefined,
          italic: kind.Shape.italic === true ? true : undefined,
          underline: kind.Shape.underline === true ? true : undefined,
        };
      }
      if (kind.Image) {
        const mime = typeof kind.Image.mime === "string" ? kind.Image.mime : "image/url";
        const isMedia = mime.startsWith("audio/") || mime.startsWith("video/");
        if (isMedia) {
          return {
            id: element.id, type: mime.startsWith("audio/") ? "audio" : "video",
            x: element.x, y: element.y, width: element.width, height: element.height,
            content: kind.Image.assetHash, color: "#0f172a", rotation, entrance, ...timing,
            hyperlink: typeof element.hyperlink === "string" ? element.hyperlink : undefined,
            groupId: normalizeGroupId(element.groupId),
            placeholder: normalizePlaceholderFlag(element.placeholder),
            mediaMime: mime.slice(0, 80),
          };
        }
        return { id: element.id, type: "image", x: element.x, y: element.y, width: element.width, height: element.height, content: kind.Image.assetHash, color: "#ffffff", rotation, entrance, ...timing, hyperlink: typeof element.hyperlink === "string" ? element.hyperlink : undefined, groupId: normalizeGroupId(element.groupId), placeholder: normalizePlaceholderFlag(element.placeholder) };
      }
      if (kind.Table) {
        const data = normalizeTableData(kind.Table.data, kind.Table.rows, kind.Table.cols);
        return {
          id: element.id,
          type: "table",
          x: element.x,
          y: element.y,
          width: element.width,
          height: element.height,
          content: "",
          rotation,
          entrance,
          ...timing,
          hyperlink: typeof element.hyperlink === "string" ? element.hyperlink : undefined,
          groupId: normalizeGroupId(element.groupId),
          placeholder: normalizePlaceholderFlag(element.placeholder),
          tableRows: data.length,
          tableCols: data[0]?.length ?? 0,
          tableData: data,
          tableMerges: normalizeTableMerges(kind.Table.merges ?? kind.Table.tableMerges, data.length, data[0]?.length ?? 0),
          tableHeaderRow: kind.Table.headerRow === true || kind.Table.tableHeaderRow === true ? true : undefined,
          tableStyle: normalizeTableStyle(kind.Table.tableStyle ?? kind.Table.style),
        };
      }
      if (kind.Chart) {
        return {
          id: element.id,
          type: "chart",
          x: element.x,
          y: element.y,
          width: element.width,
          height: element.height,
          content: "",
          rotation,
          entrance,
          ...timing,
          hyperlink: typeof element.hyperlink === "string" ? element.hyperlink : undefined,
          groupId: normalizeGroupId(element.groupId),
          placeholder: normalizePlaceholderFlag(element.placeholder),
          chartType: normalizeChartType(kind.Chart.chartType),
          chartTitle: kind.Chart.labels?.[0] || "Chart",
          chartData: normalizeChartData(kind.Chart.data),
          chartLabels: normalizeChartLabels(kind.Chart.labels?.slice(1) ?? kind.Chart.labels ?? []),
          chartLegend: kind.Chart.legend === false ? false : undefined,
          chartShowLabels: kind.Chart.showLabels === false ? false : undefined,
          chartShowAxes: kind.Chart.showAxes === false ? false : undefined,
        };
      }
      return { id: element.id, type: "rect", x: element.x ?? 100, y: element.y ?? 100, width: element.width ?? 200, height: element.height ?? 120, content: "", color: "#e2e8f0", rotation, entrance, ...timing, hyperlink: typeof element.hyperlink === "string" ? element.hyperlink : undefined, groupId: normalizeGroupId(element.groupId), placeholder: normalizePlaceholderFlag(element.placeholder) };
    }),
  }));
}

function shapeKindForElement(element: SlideElement, theme: typeof defaultTheme) {
  const isLine = element.type === "line" || element.type === "arrow";
  const strokeColor = element.lineColor || (isLine ? element.color : undefined) || theme.textColor || "#000000";
  const hasStroke = Boolean(element.lineColor) || isLine;
  return {
    Shape: {
      shapeType: element.type,
      fillColor: isLine ? "transparent" : (element.color || theme.accentColor || "#3b82f6"),
      fillGradient: element.fillGradient,
      strokeColor: hasStroke ? strokeColor : "transparent",
      strokeWidth: element.lineWidth ?? (isLine ? 3 : 0),
      shadow: element.shadow === true ? true : undefined,
      fontFamily: element.fontFamily || undefined,
      bold: element.bold === true ? true : undefined,
      italic: element.italic === true ? true : undefined,
      underline: element.underline === true ? true : undefined,
      text: element.content || undefined,
    },
  };
}

export function toDeck(slides: Slide[], source: any, theme: typeof defaultTheme, activeIndex: number, masters?: SlideMaster[]) {
  const anyFade = slides.some((slide) => slide.transition === "fade");
  const resolvedMasters = masters ?? normalizeMasters(source?.masters);
  return {
    slides: slides.map((slide) => ({
      id: slide.id,
      layout: slide.layout,
      notes: slide.notes,
      bgOverride: slide.bgOverride ?? null,
      transition: slide.transition || "none",
      masterId: slide.masterId ?? resolvedMasters[0]?.id,
      showHeader: slide.showHeader,
      showFooter: slide.showFooter,
      showDate: slide.showDate,
      showSlideNumber: slide.showSlideNumber,
      comments: normalizeSlideComments(slide.comments),
      elements: slide.elements.map((element, index) => ({
        id: element.id,
        x: element.x,
        y: element.y,
        width: element.width,
        height: element.height,
        rotation: element.rotation || 0,
        zIndex: index,
        entrance: element.entrance || "none",
        entranceDelayMs: element.entranceDelayMs ?? 0,
        entranceDurationMs: element.entranceDurationMs ?? 350,
        entranceOrder: element.entranceOrder ?? index,
        exit: element.exit || "none",
        exitDurationMs: element.exitDurationMs ?? 350,
        hyperlink: element.hyperlink,
        groupId: element.groupId,
        placeholder: element.placeholder === true ? true : undefined,
        kind: element.type === "text"
          ? {
              Text: {
                text: element.content,
                fontSize: element.fontSize || 20,
                fontFamily: element.fontFamily || theme.fontFamily,
                color: element.color || "#1e293b",
                align: element.align || "center",
                bullets: Boolean(element.bullets),
                bold: Boolean(element.bold),
                italic: Boolean(element.italic),
                underline: Boolean(element.underline),
              },
            }
          : element.type === "image"
            ? { Image: { assetHash: element.content, mime: "image/url" } }
            : element.type === "audio" || element.type === "video"
              ? {
                  Image: {
                    assetHash: element.content,
                    mime: element.mediaMime
                      || (element.type === "audio" ? "audio/mpeg" : "video/mp4"),
                  },
                }
              : element.type === "table"
              ? {
                  Table: {
                    rows: element.tableRows || element.tableData?.length || 3,
                    cols: element.tableCols || element.tableData?.[0]?.length || 3,
                    data: element.tableData || [],
                    merges: element.tableMerges ?? [],
                    headerRow: element.tableHeaderRow === true,
                    tableStyle: element.tableStyle || "plain",
                  },
                }
              : element.type === "chart"
                ? {
                    Chart: {
                      chartType: element.chartType || "bar",
                      data: element.chartData || [3, 5, 2, 8],
                      labels: [element.chartTitle || "Chart", ...(element.chartLabels || ["A", "B", "C", "D"])],
                      legend: element.chartLegend !== false,
                      showLabels: element.chartShowLabels !== false,
                      showAxes: element.chartShowAxes !== false,
                    },
                  }
                : shapeKindForElement(element, theme),
      })),
    })),
    masters: resolvedMasters,
    theme,
    canvasWidth: source?.canvasWidth || 960,
    canvasHeight: source?.canvasHeight || 540,
    activeSlideIndex: activeIndex,
    fadeBetweenSlides: anyFade,
  };
}
