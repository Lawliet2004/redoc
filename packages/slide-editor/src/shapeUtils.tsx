import type { JSX } from "solid-js";
import type { ShapeGradient, SlideElement, TableMerge, TableStyle } from "./deckNormalize";

export const SHAPE_TYPES = [
  "rect",
  "roundedRect",
  "ellipse",
  "triangle",
  "diamond",
  "star",
  "line",
  "arrow",
] as const;

export type ShapeType = typeof SHAPE_TYPES[number];

export function isShapeType(type: string): type is ShapeType {
  return (SHAPE_TYPES as readonly string[]).includes(type);
}

export function isFilledShape(type: string): boolean {
  return type !== "line" && type !== "arrow";
}

/** Grouped SmartArt-lite templates built from primitive shapes. */
export interface SmartArtTemplate {
  id: string;
  name: string;
  shapes: Array<{ type: ShapeType; x: number; y: number; width: number; height: number; color: string; text: string }>;
}

export const SMARTART_TEMPLATES: SmartArtTemplate[] = [
  {
    id: "process-3",
    name: "Process (3 steps)",
    shapes: [
      { type: "rect", x: 120, y: 200, width: 160, height: 90, color: "#3b82f6", text: "Step 1" },
      { type: "arrow", x: 290, y: 235, width: 60, height: 20, color: "#64748b", text: "" },
      { type: "rect", x: 360, y: 200, width: 160, height: 90, color: "#0d9488", text: "Step 2" },
      { type: "arrow", x: 530, y: 235, width: 60, height: 20, color: "#64748b", text: "" },
      { type: "rect", x: 600, y: 200, width: 160, height: 90, color: "#8b5cf6", text: "Step 3" },
    ],
  },
  {
    id: "cycle-3",
    name: "Cycle (3 nodes)",
    shapes: [
      { type: "ellipse", x: 400, y: 110, width: 160, height: 100, color: "#3b82f6", text: "Plan" },
      { type: "ellipse", x: 250, y: 300, width: 160, height: 100, color: "#0d9488", text: "Build" },
      { type: "ellipse", x: 550, y: 300, width: 160, height: 100, color: "#f59e0b", text: "Review" },
    ],
  },
  {
    id: "hierarchy-4",
    name: "Hierarchy (1+3)",
    shapes: [
      { type: "roundedRect", x: 380, y: 90, width: 200, height: 80, color: "#1e293b", text: "Head" },
      { type: "line", x: 480, y: 170, width: 2, height: 50, color: "#64748b", text: "" },
      { type: "roundedRect", x: 140, y: 240, width: 180, height: 76, color: "#3b82f6", text: "Team A" },
      { type: "roundedRect", x: 390, y: 240, width: 180, height: 76, color: "#0d9488", text: "Team B" },
      { type: "roundedRect", x: 640, y: 240, width: 180, height: 76, color: "#8b5cf6", text: "Team C" },
    ],
  },
  {
    id: "funnel-3",
    name: "Funnel (3 levels)",
    shapes: [
      { type: "diamond", x: 380, y: 90, width: 200, height: 100, color: "#3b82f6", text: "Top" },
      { type: "diamond", x: 410, y: 210, width: 140, height: 90, color: "#0d9488", text: "Mid" },
      { type: "diamond", x: 435, y: 320, width: 90, height: 70, color: "#f59e0b", text: "End" },
    ],
  },
];

/** Expand a SmartArt-lite template into grouped shape elements. */
export function expandSmartArt(
  template: SmartArtTemplate,
  makeId: () => string,
  groupId: string,
): SlideElement[] {
  return template.shapes.map((shape) => ({
    id: makeId(),
    type: shape.type,
    x: shape.x,
    y: shape.y,
    width: shape.width,
    height: shape.height,
    content: shape.text,
    color: shape.color,
    rotation: 0,
    groupId,
  }));
}

export function mergeTableCells(
  data: string[][],
  merges: TableMerge[],
  row: number,
  col: number,
  rowspan: number,
  colspan: number,
): TableMerge[] | null {
  const rows = data.length;
  const cols = data[0]?.length ?? 0;
  const r = Math.max(0, Math.min(rows - 1, row));
  const c = Math.max(0, Math.min(cols - 1, col));
  const rs = Math.max(1, Math.min(rows - r, Math.floor(rowspan) || 1));
  const cs = Math.max(1, Math.min(cols - c, Math.floor(colspan) || 1));
  if (rs === 1 && cs === 1) return null;
  // Reject overlaps with existing merges.
  for (let dr = 0; dr < rs; dr++) {
    for (let dc = 0; dc < cs; dc++) {
      if (isCoveredByMerge(merges, r + dr, c + dc)) return null;
    }
  }
  return [...merges, { r, c, rowspan: rs, colspan: cs }];
}

export function splitTableCell(merges: TableMerge[], row: number, col: number): TableMerge[] {
  return merges.filter((merge) => {
    const inside =
      row >= merge.r && row < merge.r + merge.rowspan &&
      col >= merge.c && col < merge.c + merge.colspan;
    return !inside;
  });
}

/** True when a cell is covered by a merge anchored at another cell. */
export function isCoveredByMerge(merges: TableMerge[], row: number, col: number): boolean {
  return merges.some(
    (merge) =>
      row >= merge.r && row < merge.r + merge.rowspan &&
      col >= merge.c && col < merge.c + merge.colspan &&
      !(merge.r === row && merge.c === col),
  );
}

/** Merge anchor lookup: the merge covering a cell, if any. */
export function mergeAt(merges: TableMerge[], row: number, col: number): TableMerge | null {
  return (
    merges.find(
      (merge) =>
        row >= merge.r && row < merge.r + merge.rowspan &&
        col >= merge.c && col < merge.c + merge.colspan,
    ) ?? null
  );
}

export function nextTableStyle(style: TableStyle | undefined): TableStyle {
  if (style === "plain") return "banded";
  if (style === "banded") return "accent-header";
  return "plain";
}

export function tableHeaderFill(style: TableStyle | undefined, accent: string): string {
  if (style === "accent-header") return accent;
  return "#1e293b";
}

/** Resolve a shape's CSS background honoring gradient fills. */
export function shapeFillCss(color: string | undefined, gradient: ShapeGradient | undefined): string {
  if (gradient) {
    const angle = Number.isFinite(gradient.angle) ? gradient.angle : 90;
    return `linear-gradient(${angle}deg, ${gradient.from}, ${gradient.to})`;
  }
  return color || "#3b82f6";
}

/** Resolve a shape's box-shadow CSS; undefined when shadows are off. */
export function shapeShadowCss(shadow: boolean | undefined): string | undefined {
  return shadow ? "0 6px 18px rgba(15, 23, 42, 0.35)" : undefined;
}

export function shapeClipPath(type: string): string | undefined {
  switch (type) {
    case "triangle":
      return "polygon(50% 0%, 0% 100%, 100% 100%)";
    case "diamond":
      return "polygon(50% 0%, 100% 50%, 50% 100%, 0% 50%)";
    case "star":
      return "polygon(50% 0%, 61% 35%, 98% 35%, 68% 57%, 79% 91%, 50% 70%, 21% 91%, 32% 57%, 2% 35%, 39% 35%)";
    default:
      return undefined;
  }
}

export function shapeBorderRadius(type: string): string | undefined {
  if (type === "roundedRect") return "16px";
  if (type === "rect") return "4px";
  if (type === "ellipse") return "50%";
  return undefined;
}

export function ShapeBody(props: {
  type: string;
  color: string;
  id: string;
  gradient?: ShapeGradient;
  strokeColor?: string;
  strokeWidth?: number;
  shadow?: boolean;
}): JSX.Element {
  const fill = props.color || "#3b82f6";
  const stroke = props.strokeColor || fill;
  const strokeWidth = Math.max(0, Math.min(24, props.strokeWidth ?? 0));
  const shadow = shapeShadowCss(props.shadow);
  const background = shapeFillCss(fill, props.gradient);
  if (props.type === "line" || props.type === "arrow") {
    return (
      <svg width="100%" height="100%" viewBox="0 0 100 100" preserveAspectRatio="none" aria-hidden="true">
        <defs>
          <marker id={`arrowhead-${props.id}`} markerWidth="8" markerHeight="8" refX="7" refY="4" orient="auto">
            <path d="M0,0 L8,4 L0,8 z" fill={fill} />
          </marker>
        </defs>
        <line
          x1="0"
          y1="50"
          x2="100"
          y2="50"
          stroke={fill}
          stroke-width="4"
          vector-effect="non-scaling-stroke"
          marker-end={props.type === "arrow" ? `url(#arrowhead-${props.id})` : undefined}
        />
      </svg>
    );
  }

  const clip = shapeClipPath(props.type);
  const radius = shapeBorderRadius(props.type);

  return (
    <div
      style={{
        width: "100%",
        height: "100%",
        background,
        "border-radius": radius,
        "clip-path": clip,
        border: strokeWidth > 0 ? `${strokeWidth}px solid ${stroke}` : undefined,
        "box-shadow": shadow,
        "box-sizing": "border-box",
      }}
    />
  );
}
