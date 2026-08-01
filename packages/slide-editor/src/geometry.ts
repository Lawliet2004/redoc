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

export type AlignMode = "left" | "center" | "right" | "top" | "middle" | "bottom";

/** Returns new positions for aligning one or more elements on the slide canvas. */
export function alignElements(
  elements: SnapElement[],
  mode: AlignMode,
  canvasWidth = 960,
  canvasHeight = 540,
): Array<{ id: string; x: number; y: number }> {
  if (!elements.length) return [];

  if (elements.length === 1) {
    const el = elements[0];
    let x = el.x;
    let y = el.y;
    if (mode === "left") x = 0;
    if (mode === "center") x = (canvasWidth - el.width) / 2;
    if (mode === "right") x = canvasWidth - el.width;
    if (mode === "top") y = 0;
    if (mode === "middle") y = (canvasHeight - el.height) / 2;
    if (mode === "bottom") y = canvasHeight - el.height;
    return [{ id: el.id, x, y }];
  }

  const minX = Math.min(...elements.map((e) => e.x));
  const maxX = Math.max(...elements.map((e) => e.x + e.width));
  const minY = Math.min(...elements.map((e) => e.y));
  const maxY = Math.max(...elements.map((e) => e.y + e.height));
  const midX = (minX + maxX) / 2;
  const midY = (minY + maxY) / 2;

  return elements.map((el) => {
    let x = el.x;
    let y = el.y;
    if (mode === "left") x = minX;
    if (mode === "center") x = midX - el.width / 2;
    if (mode === "right") x = maxX - el.width;
    if (mode === "top") y = minY;
    if (mode === "middle") y = midY - el.height / 2;
    if (mode === "bottom") y = maxY - el.height;
    return { id: el.id, x, y };
  });
}

export function assignGroupId<T extends { id: string; groupId?: string }>(
  elements: T[],
  ids: string[],
  groupId: string,
): T[] {
  return elements.map((el) => (ids.includes(el.id) ? { ...el, groupId } : el));
}

export function clearGroupIds<T extends { id: string; groupId?: string }>(
  elements: T[],
  selectedIds: string[],
): T[] {
  const selectedGroups = new Set(
    elements.filter((el) => selectedIds.includes(el.id) && el.groupId).map((el) => el.groupId),
  );
  return elements.map((el) =>
    selectedIds.includes(el.id) || (el.groupId && selectedGroups.has(el.groupId))
      ? { ...el, groupId: undefined }
      : el,
  );
}
