export interface SnapElement {
  id: string;
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface AlignmentGuide {
  axis: "x" | "y";
  position: number;
}

export interface SnapResult {
  x: number;
  y: number;
  guides: AlignmentGuide[];
}

const SNAP_DISTANCE = 6;

const nearestSnap = (
  rawStart: number,
  size: number,
  positions: number[],
): { value: number; guide: number } | null => {
  const handles = [rawStart, rawStart + size / 2, rawStart + size];
  let best: { distance: number; delta: number; guide: number } | null = null;
  for (const handle of handles) {
    for (const position of positions) {
      const distance = Math.abs(position - handle);
      if (distance <= SNAP_DISTANCE && (!best || distance < best.distance)) {
        best = { distance, delta: position - handle, guide: position };
      }
    }
  }
  return best ? { value: rawStart + best.delta, guide: best.guide } : null;
};

export function snapElementPosition(
  selected: SnapElement,
  rawX: number,
  rawY: number,
  peers: SnapElement[],
): SnapResult {
  const others = peers.filter((element) => element.id !== selected.id);
  const xPositions = [480];
  const yPositions = [270];
  for (const element of others) {
    xPositions.push(element.x, element.x + element.width / 2, element.x + element.width);
    yPositions.push(element.y, element.y + element.height / 2, element.y + element.height);
  }

  const xSnap = nearestSnap(rawX, selected.width, xPositions);
  const ySnap = nearestSnap(rawY, selected.height, yPositions);
  const x = Math.max(0, Math.min(960 - selected.width, xSnap?.value ?? rawX));
  const y = Math.max(0, Math.min(540 - selected.height, ySnap?.value ?? rawY));
  const guides: AlignmentGuide[] = [];
  if (xSnap) guides.push({ axis: "x", position: xSnap.guide });
  if (ySnap) guides.push({ axis: "y", position: ySnap.guide });
  return { x, y, guides };
}
export function computeBounds(elements: SnapElement[]) {
  if (elements.length === 0) return null;
  const minX = Math.min(...elements.map(e => e.x));
  const minY = Math.min(...elements.map(e => e.y));
  const maxX = Math.max(...elements.map(e => e.x + e.width));
  const maxY = Math.max(...elements.map(e => e.y + e.height));
  return { x: minX, y: minY, width: maxX - minX, height: maxY - minY };
}
export function hitTest(element: SnapElement, px: number, py: number): boolean {
  return px >= element.x && px <= element.x + element.width &&
         py >= element.y && py <= element.y + element.height;
}
