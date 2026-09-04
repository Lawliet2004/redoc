const CACHE_CAPACITY = 5000;
const cache = new Map<string, number>();

function cacheKey(font: string, text: string): string {
  return `${font}\0${text}`;
}

export function measureTextWidth(ctx: CanvasRenderingContext2D, font: string, text: string): number {
  const key = cacheKey(font, text);
  const existing = cache.get(key);
  if (existing !== undefined) {
    // strict LRU: move to end
    cache.delete(key);
    cache.set(key, existing);
    return existing;
  }
  ctx.font = font;
  const width = ctx.measureText(text).width;
  
  if (cache.size >= CACHE_CAPACITY) {
    // remove oldest (first item in Map)
    const oldestKey = cache.keys().next().value;
    if (oldestKey !== undefined) {
      cache.delete(oldestKey);
    }
  }
  
  cache.set(key, width);
  return width;
}

export function clearTextMeasureCache(): void {
  cache.clear();
}
