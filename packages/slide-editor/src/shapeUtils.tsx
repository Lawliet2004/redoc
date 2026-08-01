import type { JSX } from "solid-js";

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
}): JSX.Element {
  const fill = props.color || "#3b82f6";
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
        background: fill,
        "border-radius": radius,
        "clip-path": clip,
      }}
    />
  );
}
