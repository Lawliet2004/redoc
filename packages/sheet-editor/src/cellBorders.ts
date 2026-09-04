import type { BorderEdge, BorderLineStyle, CellBorders, GridCellStyle, MergeRange } from "./sheetTypes";

export const BORDER_LINE_STYLES: BorderLineStyle[] = ["thin", "medium", "thick", "dashed", "dotted", "double"];

export function normalizeBorderLineStyle(value: unknown): BorderLineStyle | null {
  return BORDER_LINE_STYLES.includes(value as BorderLineStyle) ? (value as BorderLineStyle) : null;
}

const DEFAULT_BORDER_COLOR = "#0f172a";

export function borderEdgeWidth(style: BorderLineStyle): number {
  switch (style) {
    case "medium": return 2;
    case "thick": return 3;
    case "double": return 2;
    case "dashed":
    case "dotted":
    case "thin":
    default: return 1;
  }
}

export function borderDashPattern(style: BorderLineStyle): number[] {
  switch (style) {
    case "dashed": return [4, 3];
    case "dotted": return [1, 3];
    default: return [];
  }
}

export function edgeColor(edge: BorderEdge): string {
  return edge.color || DEFAULT_BORDER_COLOR;
}

/**
 * Canvas drawing for one cell's borders. Lines are drawn inset by half the
 * line width so they stay inside the cell rect and adjacent cells don't
 * double-paint shared edges.
 */
export function drawCellBorders(
  ctx: CanvasRenderingContext2D,
  borders: CellBorders | undefined,
  x: number,
  y: number,
  w: number,
  h: number,
): void {
  if (!borders) return;
  const hLine = (edge: BorderEdge | undefined, yy: number) => {
    if (!edge) return;
    const width = borderEdgeWidth(edge.style);
    ctx.save();
    ctx.strokeStyle = edgeColor(edge);
    const pattern = borderDashPattern(edge.style);
    if (pattern.length) ctx.setLineDash(pattern);
    if (edge.style === "double") {
      const gap = Math.max(2, width);
      ctx.lineWidth = Math.max(1, Math.round(width / 3));
      for (const offset of [-gap, gap]) {
        ctx.beginPath();
        ctx.moveTo(x, yy + offset);
        ctx.lineTo(x + w, yy + offset);
        ctx.stroke();
      }
    } else {
      ctx.lineWidth = width;
      ctx.beginPath();
      ctx.moveTo(x, yy);
      ctx.lineTo(x + w, yy);
      ctx.stroke();
    }
    ctx.restore();
  };
  const vLine = (edge: BorderEdge | undefined, xx: number) => {
    if (!edge) return;
    const width = borderEdgeWidth(edge.style);
    ctx.save();
    ctx.strokeStyle = edgeColor(edge);
    const pattern = borderDashPattern(edge.style);
    if (pattern.length) ctx.setLineDash(pattern);
    if (edge.style === "double") {
      const gap = Math.max(2, width);
      ctx.lineWidth = Math.max(1, Math.round(width / 3));
      for (const offset of [-gap, gap]) {
        ctx.beginPath();
        ctx.moveTo(xx + offset, y);
        ctx.lineTo(xx + offset, y + h);
        ctx.stroke();
      }
    } else {
      ctx.lineWidth = width;
      ctx.beginPath();
      ctx.moveTo(xx, y);
      ctx.lineTo(xx, y + h);
      ctx.stroke();
    }
    ctx.restore();
  };
  hLine(borders.top, y);
  hLine(borders.bottom, y + h);
  vLine(borders.left, x);
  vLine(borders.right, x + w);
}

export type BorderPreset = "all" | "outer" | "top" | "bottom" | "none";

/**
 * Compute the per-cell borders for one cell inside a selection that received
 * `preset`. Cells on the selection edge only get outer edges there; "all"
 * adds both outer edges and inner separators.
 */
export function bordersForPresetCell(
  preset: BorderPreset,
  edge: BorderEdge | undefined,
  position: {
    isFirstRow: boolean;
    isLastRow: boolean;
    isFirstCol: boolean;
    isLastCol: boolean;
  },
): CellBorders | undefined {
  if (preset === "none") return undefined;
  const make = (): BorderEdge | undefined =>
    edge ? { style: edge.style, ...(edge.color ? { color: edge.color } : {}) } : { style: "thin" };
  if (preset === "top") {
    return position.isFirstRow ? { top: make() } : undefined;
  }
  if (preset === "bottom") {
    return position.isLastRow ? { bottom: make() } : undefined;
  }
  const outer: CellBorders = {
    ...(position.isFirstRow ? { top: make() } : {}),
    ...(position.isLastRow ? { bottom: make() } : {}),
    ...(position.isFirstCol ? { left: make() } : {}),
    ...(position.isLastCol ? { right: make() } : {}),
  };
  if (preset === "outer") {
    return Object.keys(outer).length ? outer : undefined;
  }
  // "all": outer edges plus inner separators.
  return {
    ...outer,
    ...(position.isFirstRow ? {} : { top: make() }),
    ...(position.isLastRow ? {} : { bottom: make() }),
    ...(position.isFirstCol ? {} : { left: make() }),
    ...(position.isLastCol ? {} : { right: make() }),
  };
}

/** Apply a border preset to every cell style in a rectangular range. */
export function applyBorderPresetToStyle(
  style: GridCellStyle | undefined,
  preset: BorderPreset,
  edge: BorderEdge | undefined,
  position: {
    isFirstRow: boolean;
    isLastRow: boolean;
    isFirstCol: boolean;
    isLastCol: boolean;
  },
): GridCellStyle {
  if (preset === "none") {
    if (!style?.borders) return style ?? {};
    const rest = { ...style };
    delete rest.borders;
    return rest;
  }
  return { ...style, borders: bordersForPresetCell(preset, edge, position) };
}

/** Range position helper used by the toolbar when iterating a selection. */
export function rangePosition(
  bounds: MergeRange,
  row: number,
  col: number,
): {
  isFirstRow: boolean;
  isLastRow: boolean;
  isFirstCol: boolean;
  isLastCol: boolean;
} {
  return {
    isFirstRow: row === bounds.startRow,
    isLastRow: row === bounds.endRow,
    isFirstCol: col === bounds.startCol,
    isLastCol: col === bounds.endCol,
  };
}
