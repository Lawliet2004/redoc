const CACHE_CAPACITY = 5000;
const cache = new Map<string, number>();

function cacheKey(font: string, text: string): string {
  return `${font}\0${text}`;
}

function evictHalf(): void {
  if (cache.size < CACHE_CAPACITY) return;
  const keys = [...cache.keys()].slice(0, Math.floor(CACHE_CAPACITY / 2));
  for (const key of keys) cache.delete(key);
}

/** Cached canvas text width; LRU-ish clear-half when capacity exceeded. */
export function measureTextWidth(ctx: CanvasRenderingContext2D, font: string, text: string): number {
  const key = cacheKey(font, text);
  const existing = cache.get(key);
  if (existing !== undefined) return existing;
  ctx.font = font;
  const width = ctx.measureText(text).width;
  evictHalf();
  cache.set(key, width);
  return width;
}

export function clearTextMeasureCache(): void {
  cache.clear();
}
