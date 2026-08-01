import { isShapeType, type ShapeType } from "./shapeUtils";

export const PLACEHOLDER_TITLE = "Click to add Title";
export const PLACEHOLDER_BODY = "Click to add Text";

export type SlideTransition = "none" | "fade" | "slide-left" | "slide-right";
export type ElementEntrance = "none" | "fade";

export interface SlideElement {
  id: string;
  type: "text" | ShapeType | "image" | "table" | "chart";
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
  fontFamily?: string;
  bold?: boolean;
  italic?: boolean;
  underline?: boolean;
  groupId?: string;
  tableRows?: number;
  tableCols?: number;
  tableData?: string[][];
  chartType?: "bar" | "pie";
  chartTitle?: string;
  chartData?: number[];
  chartLabels?: string[];
}

export interface Slide {
  id: string;
  title: string;
  layout: string;
  elements: SlideElement[];
  notes: string;
  transition: SlideTransition;
  bgOverride?: string | null;
}

export const defaultTheme = {
  id: "default-light",
  name: "Modern Light",
  bgColor: "#ffffff",
  textColor: "#1e293b",
  accentColor: "#3b82f6",
  fontFamily: "Inter, sans-serif",
};

export function normalizeTransition(value: unknown, fallbackFade = false): SlideTransition {
  if (value === "fade" || value === "slide-left" || value === "slide-right" || value === "none") return value;
  return fallbackFade ? "fade" : "none";
}

export function normalizeEntrance(value: unknown): ElementEntrance {
  return value === "fade" ? "fade" : "none";
}

export function normalizeTextAlign(value: unknown): "left" | "center" | "right" {
  return value === "left" || value === "right" ? value : "center";
}

export function normalizeFlatElement(element: any, rotation: number, entrance: ElementEntrance): SlideElement | null {
  const type = element.type as string;
  const base = {
    id: element.id || `el-${Math.random()}`,
    x: typeof element.x === "number" ? element.x : 100,
    y: typeof element.y === "number" ? element.y : 100,
    width: typeof element.width === "number" ? element.width : 400,
    height: typeof element.height === "number" ? element.height : 60,
    rotation,
    entrance,
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
      fontFamily: element.fontFamily,
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
  if (type === "rect" || type === "ellipse" || type === "line" || type === "arrow"
    || type === "roundedRect" || type === "triangle" || type === "diamond" || type === "star") {
    return {
      ...base,
      type,
      content: element.content ?? "",
      color: element.color ?? "#3b82f6",
      groupId: element.groupId,
    };
  }
  return null;
}

export function normalizeDeck(deck: any): Slide[] {
  if (!deck?.slides?.length) return [{
    id: "slide-1", title: "Title Slide", layout: "title", notes: "", transition: "none", elements: [
      { id: "el-1", type: "text", x: 100, y: 180, width: 760, height: 80, content: PLACEHOLDER_TITLE, fontSize: 40, color: "#1e293b", rotation: 0, align: "center" },
      { id: "el-2", type: "text", x: 150, y: 280, width: 660, height: 50, content: PLACEHOLDER_BODY, fontSize: 22, color: "#64748b", rotation: 0, align: "center" },
    ],
  }];
  const deckFade = Boolean(deck.fadeBetweenSlides);
  return deck.slides.map((slide: any) => ({
    id: slide.id,
    title: slide.title || slide.layout || "Slide",
    layout: slide.layout || "blank",
    notes: slide.notes || "",
    transition: normalizeTransition(slide.transition, deckFade),
    bgOverride: slide.bgOverride ?? slide.bg_override ?? null,
    elements: (slide.elements || []).map((element: any) => {
      const kind = element.kind || {};
      const rotation = typeof element.rotation === "number" ? element.rotation : 0;
      const entrance = normalizeEntrance(element.entrance);
      if (!element.kind && element.type) {
        const flat = normalizeFlatElement(element, rotation, entrance);
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
          groupId: element.groupId,
        };
      }
      if (kind.Image) return { id: element.id, type: "image", x: element.x, y: element.y, width: element.width, height: element.height, content: kind.Image.assetHash, color: "#ffffff", rotation, entrance };
      if (kind.Table) {
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
          tableRows: kind.Table.rows,
          tableCols: kind.Table.cols,
          tableData: kind.Table.data,
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
          chartType: kind.Chart.chartType === "pie" ? "pie" : "bar",
          chartTitle: kind.Chart.labels?.[0] || "Chart",
          chartData: kind.Chart.data,
          chartLabels: kind.Chart.labels?.slice(1) || kind.Chart.labels || [],
        };
      }
      return { id: element.id, type: "rect", x: element.x ?? 100, y: element.y ?? 100, width: element.width ?? 200, height: element.height ?? 120, content: "", color: "#e2e8f0", rotation, entrance };
    }),
  }));
}

function shapeKindForElement(element: SlideElement, theme: typeof defaultTheme) {
  const isLine = element.type === "line" || element.type === "arrow";
  const strokeColor = element.color || theme.textColor || "#000000";
  return {
    Shape: {
      shapeType: element.type,
      fillColor: isLine ? "transparent" : (element.color || theme.accentColor || "#3b82f6"),
      strokeColor: isLine ? strokeColor : "transparent",
      strokeWidth: isLine ? 3 : 0,
      text: element.content || undefined,
    },
  };
}

export function toDeck(slides: Slide[], source: any, theme: typeof defaultTheme, activeIndex: number) {
  const anyFade = slides.some((slide) => slide.transition === "fade");
  return {
    slides: slides.map((slide) => ({
      id: slide.id,
      layout: slide.layout,
      notes: slide.notes,
      bgOverride: slide.bgOverride ?? null,
      transition: slide.transition || "none",
      elements: slide.elements.map((element, index) => ({
        id: element.id,
        x: element.x,
        y: element.y,
        width: element.width,
        height: element.height,
        rotation: element.rotation || 0,
        zIndex: index,
        entrance: element.entrance || "none",
        groupId: element.groupId,
        kind: element.type === "text"
          ? {
              Text: {
                text: element.content,
                fontSize: element.fontSize || 20,
                fontFamily: element.fontFamily || "Inter, sans-serif",
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
            : element.type === "table"
              ? {
                  Table: {
                    rows: element.tableRows || element.tableData?.length || 3,
                    cols: element.tableCols || element.tableData?.[0]?.length || 3,
                    data: element.tableData || [],
                  },
                }
              : element.type === "chart"
                ? {
                    Chart: {
                      chartType: element.chartType || "bar",
                      data: element.chartData || [3, 5, 2, 8],
                      labels: [element.chartTitle || "Chart", ...(element.chartLabels || ["A", "B", "C", "D"])],
                    },
                  }
                : shapeKindForElement(element, theme),
      })),
    })),
    theme,
    canvasWidth: source?.canvasWidth || 960,
    canvasHeight: source?.canvasHeight || 540,
    activeSlideIndex: activeIndex,
    fadeBetweenSlides: anyFade,
  };
}
